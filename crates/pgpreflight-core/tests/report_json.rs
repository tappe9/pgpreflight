use pgpreflight_core::{Report, StatementKind};

#[test]
fn clean_report_serializes_as_schema_v1() {
    let report = Report::clean(StatementKind::Select);
    let value = serde_json::to_value(report).expect("report should serialize");

    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["status"], "clean");
    assert_eq!(value["statement"]["kind"], "select");
    assert_eq!(value["summary"]["errors"], 0);
    assert_eq!(value["summary"]["warnings"], 0);
    assert_eq!(value["diagnostics"], serde_json::json!([]));
}

#[test]
fn clean_report_matches_json_schema_v1() {
    let schema_text = include_str!("../../../schemas/report-v1.schema.json");
    let schema_json: serde_json::Value =
        serde_json::from_str(schema_text).expect("schema should be valid JSON");
    let validator = jsonschema::validator_for(&schema_json).expect("schema should compile");

    let report = serde_json::to_value(Report::clean(StatementKind::Select))
        .expect("report should serialize");

    assert!(validator.is_valid(&report));
}
