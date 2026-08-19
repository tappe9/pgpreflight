use serde::{Deserialize, Serialize};

use crate::{RelationRef, StatementKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuleId {
    PGP001,
    PGP002,
    PGP101,
    PGP102,
    PGP103,
    PGP104,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiagnosticEvidence {
    MissingWhere {
        relation: RelationRef,
        estimated_affected_rows: Option<f64>,
    },
    LargeAffectedRows {
        relation: RelationRef,
        estimated_affected_rows: f64,
        estimated_relation_rows: Option<f64>,
        estimated_relation_ratio: Option<f64>,
        triggered_by: Vec<AffectedRowsTrigger>,
    },
    LargeSequentialScan {
        relation: RelationRef,
        alias: Option<String>,
        estimated_scanned_rows: f64,
        estimated_output_rows: f64,
        estimated_output_ratio: f64,
    },
    LargeResultSet {
        estimated_result_rows: f64,
    },
    CartesianJoin {
        disconnected_groups: Vec<Vec<RelationRef>>,
        estimated_result_rows: Option<f64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AffectedRowsTrigger {
    AbsoluteRows,
    RelationRatio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub rule_id: RuleId,
    pub severity: Severity,
    pub title: String,
    pub message: String,
    pub evidence: DiagnosticEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thresholds: Option<DiagnosticThresholds>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DiagnosticThresholds {
    LargeAffectedRows {
        max_rows: f64,
        max_table_ratio: f64,
        min_rows_for_ratio: f64,
    },
    LargeSequentialScan {
        min_relation_rows: f64,
        max_output_ratio: f64,
    },
    LargeResultSet {
        max_result_rows: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportStatus {
    Clean,
    Warnings,
    Errors,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatementSummary {
    pub kind: StatementKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSummary {
    pub errors: u32,
    pub warnings: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureInfo {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub tool: ToolInfo,
    pub status: ReportStatus,
    pub statement: Option<StatementSummary>,
    pub summary: ReportSummary,
    pub diagnostics: Vec<Diagnostic>,
    pub failure: Option<FailureInfo>,
}

impl Report {
    pub fn clean(kind: StatementKind) -> Self {
        Self {
            schema_version: 1,
            tool: ToolInfo {
                name: "pgpreflight".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            status: ReportStatus::Clean,
            statement: Some(StatementSummary { kind }),
            summary: ReportSummary {
                errors: 0,
                warnings: 0,
            },
            diagnostics: Vec::new(),
            failure: None,
        }
    }
}
