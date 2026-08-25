use std::{collections::BTreeSet, fmt};

use pgpreflight_core::{
    AnalysisInput, NormalizedPlan, PlanNode, PostgresConfig, RelationRef, RelationStats,
};
use serde_json::Value;
use thiserror::Error;
use tokio_postgres::{Client, NoTls, Transaction, error::SqlState};

use crate::{ValidatedStatement, normalization::normalize_plan};

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
    #[error("PostgreSQL catalog evidence failed")]
    Catalog,
    #[error("Safe Mode rollback failed")]
    Rollback,
}

pub struct PlannedStatement {
    analysis: AnalysisInput,
}

impl PlannedStatement {
    pub fn analysis_input(&self) -> &AnalysisInput {
        &self.analysis
    }
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
        let normalized_plan = match normalize_plan(&raw_plan, statement.facts().kind) {
            Ok(plan) => plan,
            Err(_) => return rollback_error(transaction, PlanningError::InvalidPlan).await,
        };
        let relations = match load_relation_stats(&transaction, &normalized_plan).await {
            Ok(relations) => relations,
            Err(error) => return rollback_error(transaction, error).await,
        };
        let analysis = AnalysisInput {
            statement: statement.facts().clone(),
            plan: normalized_plan,
            relations,
        };

        transaction
            .rollback()
            .await
            .map_err(|_| PlanningError::Rollback)?;

        Ok(PlannedStatement { analysis })
    }
}

async fn load_relation_stats(
    transaction: &Transaction<'_>,
    plan: &NormalizedPlan,
) -> Result<Vec<RelationStats>, PlanningError> {
    const RELATION_STATS_SQL: &str = "\
        SELECT c.reltuples::float8, c.relpages::bigint \
        FROM pg_catalog.pg_class AS c \
        JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace \
        WHERE n.nspname = $1 AND c.relname = $2";

    let relations = collect_relations(plan);
    let mut stats = Vec::with_capacity(relations.len());

    for relation in relations {
        let row = transaction
            .query_opt(RELATION_STATS_SQL, &[&relation.schema, &relation.name])
            .await
            .map_err(|error| classify_catalog_error(&error))?;

        let (estimated_live_rows, pages) = match row {
            Some(row) => {
                let rows = row
                    .try_get::<_, f64>(0)
                    .map_err(|_| PlanningError::Catalog)?;
                let pages = row
                    .try_get::<_, i64>(1)
                    .map_err(|_| PlanningError::Catalog)?;
                (nonnegative_finite(rows), u64::try_from(pages).ok())
            }
            None => (None, None),
        };

        stats.push(RelationStats {
            relation,
            estimated_live_rows,
            pages,
        });
    }

    Ok(stats)
}

fn collect_relations(plan: &NormalizedPlan) -> Vec<RelationRef> {
    let mut seen = BTreeSet::new();
    let mut relations = Vec::new();
    collect_node_relations(&plan.root, &mut seen, &mut relations);
    relations
}

fn collect_node_relations(
    node: &PlanNode,
    seen: &mut BTreeSet<RelationRef>,
    relations: &mut Vec<RelationRef>,
) {
    if let Some(relation) = &node.relation
        && seen.insert(relation.clone())
    {
        relations.push(relation.clone());
    }

    for child in &node.children {
        collect_node_relations(child, seen, relations);
    }
}

fn nonnegative_finite(value: f64) -> Option<f64> {
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn classify_planning_error(error: &tokio_postgres::Error) -> PlanningError {
    if is_timeout(error) {
        PlanningError::Timeout
    } else {
        PlanningError::Planning
    }
}

fn classify_catalog_error(error: &tokio_postgres::Error) -> PlanningError {
    if is_timeout(error) {
        PlanningError::Timeout
    } else {
        PlanningError::Catalog
    }
}

fn is_timeout(error: &tokio_postgres::Error) -> bool {
    matches!(
        error.as_db_error().map(|error| error.code()),
        Some(&SqlState::QUERY_CANCELED | &SqlState::LOCK_NOT_AVAILABLE)
    )
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
