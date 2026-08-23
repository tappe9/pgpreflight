use sqlparser::{ast::Statement, dialect::PostgreSqlDialect, parser::Parser};

use crate::CheckError;

pub(crate) fn parse_single_statement(sql: &str) -> Result<Statement, CheckError> {
    let dialect = PostgreSqlDialect {};
    let mut statements = Parser::parse_sql(&dialect, sql).map_err(|_| CheckError::SqlParse)?;

    if statements.len() != 1 {
        return Err(CheckError::MultipleStatements);
    }

    Ok(statements.remove(0))
}
