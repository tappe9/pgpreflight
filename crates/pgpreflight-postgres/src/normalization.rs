#[cfg(test)]
mod tests {
    use pgpreflight_core::{PlanNodeKind, RelationRef, StatementKind};
    use serde_json::json;

    use super::normalize_plan;

    #[test]
    fn normalizes_seq_scan_without_expression_payloads() {
        let raw = json!([{
            "Plan": {
                "Node Type": "Seq Scan",
                "Relation Name": "widgets",
                "Schema": "public",
                "Alias": "w",
                "Startup Cost": 0.0,
                "Total Cost": 12.5,
                "Plan Rows": 42,
                "Filter": "(payload = 'literal-secret-marker')",
                "Output": ["id", "payload"]
            }
        }]);

        let plan = normalize_plan(&raw, StatementKind::Select).unwrap();

        assert_eq!(plan.root.kind, PlanNodeKind::SeqScan);
        assert_eq!(
            plan.root.relation,
            Some(RelationRef::new("public", "widgets"))
        );
        assert_eq!(plan.root.relation_alias.as_deref(), Some("w"));
        assert_eq!(plan.root.estimated_rows, 42.0);
        assert_eq!(plan.estimated_affected_rows, None);

        let serialized = serde_json::to_string(&plan).unwrap();
        assert!(!serialized.contains("literal-secret-marker"));
        assert!(!serialized.contains("Filter"));
        assert!(!serialized.contains("Output"));
    }

    #[test]
    fn normalizes_index_scan() {
        let raw = json!([{
            "Plan": {
                "Node Type": "Index Scan",
                "Relation Name": "widgets",
                "Schema": "app",
                "Alias": "widgets",
                "Startup Cost": 0.15,
                "Total Cost": 8.17,
                "Plan Rows": 1,
                "Index Cond": "(id = 7)"
            }
        }]);

        let plan = normalize_plan(&raw, StatementKind::Select).unwrap();

        assert_eq!(plan.root.kind, PlanNodeKind::IndexScan);
        assert_eq!(plan.root.relation, Some(RelationRef::new("app", "widgets")));
        assert_eq!(plan.root.estimated_rows, 1.0);
    }

    #[test]
    fn normalizes_limit_aggregate_append_and_unknown_nodes() {
        let raw = json!([{
            "Plan": {
                "Node Type": "Limit",
                "Startup Cost": 1.0,
                "Total Cost": 2.0,
                "Plan Rows": 5,
                "Plans": [{
                    "Node Type": "Aggregate",
                    "Startup Cost": 1.0,
                    "Total Cost": 3.0,
                    "Plan Rows": 10,
                    "Plans": [{
                        "Node Type": "Append",
                        "Startup Cost": 0.0,
                        "Total Cost": 4.0,
                        "Plan Rows": 20,
                        "Plans": [{
                            "Node Type": "Seq Scan",
                            "Relation Name": "left_part",
                            "Schema": "public",
                            "Alias": "left_part",
                            "Startup Cost": 0.0,
                            "Total Cost": 1.0,
                            "Plan Rows": 10
                        }, {
                            "Node Type": "Future Custom Node",
                            "Startup Cost": 0.0,
                            "Total Cost": 1.0,
                            "Plan Rows": 10,
                            "Custom Expression": "secret-extension-payload"
                        }]
                    }]
                }]
            }
        }]);

        let plan = normalize_plan(&raw, StatementKind::Select).unwrap();
        let aggregate = &plan.root.children[0];
        let append = &aggregate.children[0];
        let unknown = &append.children[1];

        assert_eq!(plan.root.kind, PlanNodeKind::Limit);
        assert_eq!(aggregate.kind, PlanNodeKind::Aggregate);
        assert_eq!(append.kind, PlanNodeKind::Append);
        assert_eq!(
            unknown.kind,
            PlanNodeKind::Other("Future Custom Node".to_owned())
        );

        let serialized = serde_json::to_string(&plan).unwrap();
        assert!(!serialized.contains("secret-extension-payload"));
        assert!(!serialized.contains("Custom Expression"));
    }

    #[test]
    fn derives_update_and_delete_affected_rows_from_modify_table_input() {
        let raw = json!([{
            "Plan": {
                "Node Type": "ModifyTable",
                "Relation Name": "widgets",
                "Schema": "public",
                "Alias": "widgets",
                "Startup Cost": 0.0,
                "Total Cost": 20.0,
                "Plan Rows": 0,
                "Plans": [{
                    "Node Type": "Seq Scan",
                    "Relation Name": "widgets",
                    "Schema": "public",
                    "Alias": "widgets",
                    "Startup Cost": 0.0,
                    "Total Cost": 18.0,
                    "Plan Rows": 12
                }]
            }
        }]);

        let update = normalize_plan(&raw, StatementKind::Update).unwrap();
        let delete = normalize_plan(&raw, StatementKind::Delete).unwrap();

        assert_eq!(update.root.kind, PlanNodeKind::ModifyTable);
        assert_eq!(update.estimated_affected_rows, Some(12.0));
        assert_eq!(delete.estimated_affected_rows, Some(12.0));
    }

    #[test]
    fn unsupported_shapes_fail_instead_of_guessing_relation_identity() {
        let missing_schema = json!([{
            "Plan": {
                "Node Type": "Seq Scan",
                "Relation Name": "widgets",
                "Startup Cost": 0.0,
                "Total Cost": 1.0,
                "Plan Rows": 1
            }
        }]);
        let wrong_top_level = json!({"Plan": {}});

        assert!(normalize_plan(&missing_schema, StatementKind::Select).is_err());
        assert!(normalize_plan(&wrong_top_level, StatementKind::Select).is_err());
    }
}
