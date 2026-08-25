use pgpreflight_core::{NormalizedPlan, PlanNode, PlanNodeKind, RelationRef, StatementKind};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NormalizationError;

pub(crate) fn normalize_plan(
    raw_plan: &Value,
    statement_kind: StatementKind,
) -> Result<NormalizedPlan, NormalizationError> {
    let documents = raw_plan.as_array().ok_or(NormalizationError)?;
    let [document] = documents.as_slice() else {
        return Err(NormalizationError);
    };
    let document = document.as_object().ok_or(NormalizationError)?;
    let root = document.get("Plan").ok_or(NormalizationError)?;
    let root = normalize_node(root)?;
    let estimated_affected_rows = affected_rows(&root, statement_kind);

    Ok(NormalizedPlan {
        root,
        estimated_affected_rows,
    })
}

fn normalize_node(raw_node: &Value) -> Result<PlanNode, NormalizationError> {
    let node = raw_node.as_object().ok_or(NormalizationError)?;
    let node_type = required_string(node, "Node Type")?;
    let relation = normalize_relation(node)?;
    let relation_alias = optional_string(node, "Alias")?;
    let children = normalize_children(node)?;

    Ok(PlanNode {
        kind: normalize_node_kind(node_type),
        estimated_rows: required_nonnegative_number(node, "Plan Rows")?,
        startup_cost: required_nonnegative_number(node, "Startup Cost")?,
        total_cost: required_nonnegative_number(node, "Total Cost")?,
        relation,
        relation_alias,
        children,
    })
}

fn normalize_relation(node: &Map<String, Value>) -> Result<Option<RelationRef>, NormalizationError> {
    let schema = optional_string(node, "Schema")?;
    let relation = optional_string(node, "Relation Name")?;

    match (schema, relation) {
        (None, None) => Ok(None),
        (Some(schema), Some(relation)) => Ok(Some(RelationRef::new(schema, relation))),
        _ => Err(NormalizationError),
    }
}

fn normalize_children(node: &Map<String, Value>) -> Result<Vec<PlanNode>, NormalizationError> {
    match node.get("Plans") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(children)) => children.iter().map(normalize_node).collect(),
        Some(_) => Err(NormalizationError),
    }
}

fn normalize_node_kind(node_type: &str) -> PlanNodeKind {
    match node_type {
        "Seq Scan" => PlanNodeKind::SeqScan,
        "Index Scan" => PlanNodeKind::IndexScan,
        "Index Only Scan" => PlanNodeKind::IndexOnlyScan,
        "Bitmap Heap Scan" => PlanNodeKind::BitmapHeapScan,
        "Bitmap Index Scan" => PlanNodeKind::BitmapIndexScan,
        "Nested Loop" => PlanNodeKind::NestedLoop,
        "Hash Join" => PlanNodeKind::HashJoin,
        "Merge Join" => PlanNodeKind::MergeJoin,
        "ModifyTable" => PlanNodeKind::ModifyTable,
        "Append" => PlanNodeKind::Append,
        "Gather" => PlanNodeKind::Gather,
        "Limit" => PlanNodeKind::Limit,
        "Aggregate" => PlanNodeKind::Aggregate,
        other => PlanNodeKind::Other(other.to_owned()),
    }
}

fn affected_rows(root: &PlanNode, statement_kind: StatementKind) -> Option<f64> {
    if !matches!(statement_kind, StatementKind::Update | StatementKind::Delete)
        || root.kind != PlanNodeKind::ModifyTable
    {
        return None;
    }

    let [input] = root.children.as_slice() else {
        return None;
    };

    Some(input.estimated_rows)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, NormalizationError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(NormalizationError)
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, NormalizationError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(NormalizationError),
    }
}

fn required_nonnegative_number(
    object: &Map<String, Value>,
    key: &str,
) -> Result<f64, NormalizationError> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or(NormalizationError)?;

    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(NormalizationError)
    }
}

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
