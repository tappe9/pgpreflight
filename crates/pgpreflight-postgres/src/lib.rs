#![forbid(unsafe_code)]

mod error;
mod join_graph;
mod normalization;
mod parser;
mod planning;
mod validation;

pub use error::CheckError;
pub use planning::{PlannedStatement, PlanningError, SafeModePlanner};
pub use validation::ValidatedStatement;

pub fn parse_and_validate(sql: &str) -> Result<ValidatedStatement, CheckError> {
    let statement = parser::parse_single_statement(sql)?;
    validation::validate_statement(statement)
}

pub const CRATE_NAME: &str = "pgpreflight-postgres";
