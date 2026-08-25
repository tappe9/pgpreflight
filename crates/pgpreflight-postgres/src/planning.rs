use std::fmt;

use pgpreflight_core::PostgresConfig;
use serde_json::Value;
use thiserror::Error;
use tokio_postgres::{Client, NoTls, Transaction, error::SqlState};

use crate::ValidatedStatement;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PlanningError {
    #[error("database connection failed")]
    Connection,
    #[error("Safe Mode transaction could not be started")]
    Transaction,
    #[error("Safe Mode transaction configuration failed")]
    Configuration,
    #[error("PostgreSQL planning timed out")]
    Timeout,
    #[error("PostgreSQL planning failed")]
    Planning,
    #[error("PostgreSQL returned an invalid plan response")]
    InvalidPlan,
    #[error("Safe Mode rollback failed")]
    Rollback,
}

pub struct PlannedStatement {
    pub(crate) raw_plan: Value,
}

impl fmt::Debug for PlannedStatement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlannedStatement")
            .finish_non_exhaustive()
    }
}

pub struct SafeModePlanner {
    client: Client,
}

impl SafeModePlanner {
    pub async fn connect(database_url: &str) -> Result<Self, PlanningError> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .map_err(|_| PlanningError::Connection)?;

        tokio::spawn(async move {
            let _ = connection.await;
        });

        Ok(Self { client })
    }

    pub async fn plan(
        &mut self,
        statement: &ValidatedStatement,
        config: &PostgresConfig,
    ) -> Result<PlannedStatement, PlanningError> {
        let transaction = self
            .client
            .build_transaction()
            .read_only(true)
            .start()
            .await
            .map_err(|_| PlanningError::Transaction)?;

        let setup_sql = format!(
            "SET LOCAL statement_timeout = '{}ms'; SET LOCAL lock_timeout = '{}ms';",
            config.statement_timeout_ms, config.lock_timeout_ms
        );
        if transaction.batch_execute(&setup_sql).await.is_err() {
            return rollback_error(transaction, PlanningError::Configuration).await;
        }

        let explain_sql = format!(
            "EXPLAIN (FORMAT JSON, VERBOSE TRUE) {}",
            statement.statement()
        );
        let row = match transaction.query_one(&explain_sql, &[]).await {
            Ok(row) => row,
            Err(error) => {
                let error = classify_planning_error(&error);
                return rollback_error(transaction, error).await;
            }
        };

        let raw_plan = match row.try_get::<_, Value>(0) {
            Ok(raw_plan) => raw_plan,
            Err(_) => return rollback_error(transaction, PlanningError::InvalidPlan).await,
        };

        transaction
            .rollback()
            .await
            .map_err(|_| PlanningError::Rollback)?;

        Ok(PlannedStatement { raw_plan })
    }
}

fn classify_planning_error(error: &tokio_postgres::Error) -> PlanningError {
    match error.as_db_error().map(|error| error.code()) {
        Some(&SqlState::QUERY_CANCELED | &SqlState::LOCK_NOT_AVAILABLE) => PlanningError::Timeout,
        _ => PlanningError::Planning,
    }
}

async fn rollback_error(
    transaction: Transaction<'_>,
    primary_error: PlanningError,
) -> Result<PlannedStatement, PlanningError> {
    transaction
        .rollback()
        .await
        .map_err(|_| PlanningError::Rollback)?;
    Err(primary_error)
}
