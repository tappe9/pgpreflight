use pgpreflight_core::{
    Diagnostic, DiagnosticEvidence, DiagnosticThresholds, FailureInfo, Report, ReportStatus,
    ReportSummary, RuleId, Severity, StatementKind, StatementSummary, ToolInfo,
};

fn assert_matches_schema_v1(report: &Report) {
    let schema_text = include_str!("../../../schemas/report-v1.schema.json");
    let schema_json: serde_json::Value =
        serde_json::from_str(schema_text).expect("schema should be valid JSON");
    let validator = jsonschema::validator_for(&schema_json).expect("schema should compile");
    let value = serde_json::to_value(report).expect("report should serialize");

    assert!(
        validator.is_valid(&value),
        "report did not match schema v1: {value:#}"
    );
}

fn tool() -> ToolInfo {
    ToolInfo {
        name: "pgpreflight".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[test]
fn clean_report_serializes_as_schema_v1() {
    let report = Report::clean(StatementKind::Select);
    let value = serde_json::to_value(&report).expect("report should serialize");

    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["status"], "clean");
    assert_eq!(value["statement"]["kind"], "select");
    assert_eq!(value["summary"]["errors"], 0);
    assert_eq!(value["summary"]["warnings"], 0);
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert_matches_schema_v1(&report);
}

#[test]
fn warning_report_matches_json_schema_v1() {
    let report = Report {
        schema_version: 1,
        tool: tool(),
        status: ReportStatus::Warnings,
        statement: Some(StatementSummary {
            kind: StatementKind::Select,
        }),
        summary: ReportSummary {
            errors: 0,
            warnings: 1,
        },
        diagnostics: vec![Diagnostic {
            rule_id: RuleId::PGP103,
            severity: Severity::Warning,
            title: "Large result set".to_owned(),
            message: "The estimated result set exceeds the configured threshold.".to_owned(),
            evidence: DiagnosticEvidence::LargeResultSet {
                estimated_result_rows: 20_000.0,
            },
            thresholds: Some(DiagnosticThresholds::LargeResultSet {
                max_result_rows: 10_000.0,
            }),
        }],
        failure: None,
    };

    assert_matches_schema_v1(&report);
}

#[test]
fn error_diagnostic_report_matches_json_schema_v1() {
    let report = Report {
        schema_version: 1,
        tool: tool(),
        status: ReportStatus::Errors,
        statement: Some(StatementSummary {
            kind: StatementKind::Update,
        }),
        summary: ReportSummary {
            errors: 1,
            warnings: 0,
        },
        diagnostics: vec![Diagnostic {
            rule_id: RuleId::PGP001,
            severity: Severity::Error,
            title: "UPDATE without WHERE".to_owned(),
            message: "The UPDATE statement has no WHERE clause.".to_owned(),
            evidence: DiagnosticEvidence::MissingWhere {
                relation: pgpreflight_core::RelationRef::new("public", "orders"),
                estimated_affected_rows: Some(100.0),
            },
            thresholds: None,
        }],
        failure: None,
    };

    assert_matches_schema_v1(&report);
}

#[test]
fn tool_failure_report_matches_json_schema_v1() {
    let report = Report {
        schema_version: 1,
        tool: tool(),
        status: ReportStatus::Failed,
        statement: None,
        summary: ReportSummary {
            errors: 0,
            warnings: 0,
        },
        diagnostics: Vec::new(),
        failure: Some(FailureInfo {
            kind: "database_connection".to_owned(),
            message: "database connection failed.".to_owned(),
        }),
    };

    assert_matches_schema_v1(&report);
}
