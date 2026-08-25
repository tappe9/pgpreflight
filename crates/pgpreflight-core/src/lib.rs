#![forbid(unsafe_code)]

mod config;
mod diagnostic;
mod model;
mod rules;

pub use config::{
    Config, ConfigError, LargeAffectedConfig, LargeResultConfig, LargeSequentialScanConfig,
    PostgresConfig, RulesConfig, ToggleRuleConfig,
};
pub use diagnostic::{
    AffectedRowsTrigger, Diagnostic, DiagnosticEvidence, DiagnosticThresholds, FailureInfo, Report,
    ReportStatus, ReportSummary, RuleId, Severity, StatementSummary, ToolInfo,
};
pub use model::{
    AnalysisInput, JoinEdge, JoinGraph, NormalizedPlan, PlanNode, PlanNodeKind, RelationOccurrence,
    RelationRef, RelationStats, StatementFacts, StatementKind,
};
pub use rules::analyze;

pub const CRATE_NAME: &str = "pgpreflight-core";
