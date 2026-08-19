use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CheckError {
    #[error("SQL could not be parsed")]
    SqlParse,
    #[error("exactly one SQL statement is required")]
    MultipleStatements,
    #[error("statement type is not supported")]
    UnsupportedStatement,
    #[error("unsafe SQL construct: {kind}")]
    UnsafeConstruct { kind: &'static str },
}
