use pgpreflight_core::{AnalysisInput, Config, RuleId, analyze};
use serde_json::{Value, json};

fn analysis_input(
    kind: &str,
    estimated_affected_rows: Option<f64>,
    graph: Value,
) -> AnalysisInput {
    let target_relation = if kind == "select" {
        Value::Null
    } else {
        json!({ "schema": "public", "name": "accounts" })
    };
    let root_kind = if kind == "select" {
        "nested_loop"
    } else {
        "modify_table"
    };

    serde_json::from_value(json!({
        "statement": {
            "kind": kind,
            "target_relation": target_relation,
            "has_where": true,
            "has_returning": false,
            "join_graph": graph
        },
        "plan": {
            "root": {
                "kind": root_kind,
                "estimated_rows": 999.0,
                "startup_cost": 0.0,
                "total_cost": 10.0,
                "relation": target_relation,
                "relation_alias": "a",
                "children": []
            },
            "estimated_affected_rows": estimated_affected_rows
        },
        "relations": []
    }))
    .expect("analysis input fixture should deserialize")
}

fn disconnected_graph() -> Value {
    json!({
        "relation_occurrences": [
            {
                "relation": { "schema": "public", "name": "accounts" },
                "alias": "a"
            },
            {
                "relation": { "schema": "public", "name": "orders" },
                "alias": "o"
            }
        ],
        "edges": [],
        "indeterminate": false
    })
}

fn pgp104_evidence(input: &AnalysisInput) -> Value {
    let report = analyze(input, &Config::default());
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule_id == RuleId::PGP104)
        .expect("disconnected graph should emit PGP104");

    serde_json::to_value(&diagnostic.evidence).expect("evidence should serialize")
}

#[test]
fn dml_evidence_uses_affected_rows_and_omits_unknown_estimates() {
    for kind in ["update", "delete"] {
        let evidence = pgp104_evidence(&analysis_input(kind, Some(12.0), disconnected_graph()));
        assert_eq!(evidence["estimated_rows"], json!(12.0), "{kind}");
    }

    let evidence = pgp104_evidence(&analysis_input("delete", None, disconnected_graph()));
    assert!(evidence.get("estimated_rows").is_none());
}

#[test]
fn malformed_edges_fail_closed_without_a_pgp104_diagnostic() {
    let mut graph = disconnected_graph();
    graph["edges"] = json!([{ "left": 0, "right": 2 }]);

    let report = analyze(
        &analysis_input("select", None, graph),
        &Config::default(),
    );

    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule_id != RuleId::PGP104)
    );
}
