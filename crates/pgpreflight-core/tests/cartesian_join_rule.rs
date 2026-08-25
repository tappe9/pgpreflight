use pgpreflight_core::{AnalysisInput, Config, ReportStatus, RuleId, analyze};
use serde_json::{Value, json};

fn analysis_input(graph: Value, estimated_result_rows: f64) -> AnalysisInput {
    serde_json::from_value(json!({
        "statement": {
            "kind": "select",
            "target_relation": null,
            "has_where": true,
            "has_returning": false,
            "join_graph": graph
        },
        "plan": {
            "root": {
                "kind": "nested_loop",
                "estimated_rows": estimated_result_rows,
                "startup_cost": 0.0,
                "total_cost": 10.0,
                "relation": null,
                "relation_alias": null,
                "children": []
            },
            "estimated_affected_rows": null
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
            },
            {
                "relation": { "schema": "public", "name": "products" },
                "alias": "p"
            },
            {
                "relation": { "schema": "public", "name": "line_items" },
                "alias": "li"
            }
        ],
        "edges": [
            { "left": 3, "right": 2 },
            { "left": 1, "right": 0 },
            { "left": 0, "right": 1 }
        ],
        "indeterminate": false
    })
}

#[test]
fn disconnected_components_emit_pgp104_with_safe_deterministic_evidence() {
    let report = analyze(
        &analysis_input(disconnected_graph(), 42.0),
        &Config::default(),
    );

    assert_eq!(report.status, ReportStatus::Warnings);
    assert_eq!(report.summary.errors, 0);
    assert_eq!(report.summary.warnings, 1);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].rule_id, RuleId::PGP104);

    let evidence = serde_json::to_value(&report.diagnostics[0].evidence)
        .expect("diagnostic evidence should serialize");
    assert_eq!(
        evidence,
        json!({
            "kind": "cartesian_join",
            "disconnected_groups": [
                [
                    {
                        "relation": { "schema": "public", "name": "accounts" },
                        "alias": "a"
                    },
                    {
                        "relation": { "schema": "public", "name": "orders" },
                        "alias": "o"
                    }
                ],
                [
                    {
                        "relation": { "schema": "public", "name": "products" },
                        "alias": "p"
                    },
                    {
                        "relation": { "schema": "public", "name": "line_items" },
                        "alias": "li"
                    }
                ]
            ],
            "estimated_rows": 42.0
        })
    );
}

#[test]
fn connected_indeterminate_and_single_relation_graphs_skip_pgp104() {
    let graphs = [
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
            "edges": [{ "left": 0, "right": 1 }],
            "indeterminate": false
        }),
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
            "indeterminate": true
        }),
        json!({
            "relation_occurrences": [
                {
                    "relation": { "schema": "public", "name": "accounts" },
                    "alias": null
                }
            ],
            "edges": [],
            "indeterminate": false
        }),
    ];

    for graph in graphs {
        let report = analyze(&analysis_input(graph, 1.0), &Config::default());
        assert!(
            report
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule_id != RuleId::PGP104)
        );
    }
}

#[test]
fn pgp104_ordering_and_summary_are_deterministic() {
    let mut config = Config::default();
    config.rules.pgp103.max_result_rows = 42.0;

    let report = analyze(&analysis_input(disconnected_graph(), 42.0), &config);

    assert_eq!(report.status, ReportStatus::Warnings);
    assert_eq!(report.summary.errors, 0);
    assert_eq!(report.summary.warnings, 2);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.rule_id)
            .collect::<Vec<_>>(),
        vec![RuleId::PGP103, RuleId::PGP104]
    );
}

#[test]
fn disabled_pgp104_does_not_emit_a_diagnostic() {
    let mut config = Config::default();
    config.rules.pgp104.enabled = false;

    let report = analyze(&analysis_input(disconnected_graph(), 1.0), &config);

    assert_eq!(report.status, ReportStatus::Clean);
    assert!(report.diagnostics.is_empty());
}
