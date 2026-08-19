use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatementKind {
    Select,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RelationRef {
    pub schema: String,
    pub name: String,
}

impl RelationRef {
    pub fn new(schema: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema: schema.into(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct JoinGraph {
    pub relation_occurrences: Vec<RelationOccurrence>,
    pub edges: Vec<JoinEdge>,
    pub indeterminate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationOccurrence {
    pub relation: RelationRef,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinEdge {
    pub left: usize,
    pub right: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatementFacts {
    pub kind: StatementKind,
    pub target_relation: Option<RelationRef>,
    pub has_where: bool,
    pub has_returning: bool,
    pub join_graph: JoinGraph,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPlan {
    pub root: PlanNode,
    pub estimated_affected_rows: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanNode {
    pub kind: PlanNodeKind,
    pub estimated_rows: f64,
    pub startup_cost: f64,
    pub total_cost: f64,
    pub relation: Option<RelationRef>,
    pub relation_alias: Option<String>,
    pub children: Vec<PlanNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanNodeKind {
    SeqScan,
    IndexScan,
    IndexOnlyScan,
    BitmapHeapScan,
    BitmapIndexScan,
    NestedLoop,
    HashJoin,
    MergeJoin,
    ModifyTable,
    Append,
    Gather,
    Limit,
    Aggregate,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationStats {
    pub relation: RelationRef,
    pub estimated_live_rows: Option<f64>,
    pub pages: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisInput {
    pub statement: StatementFacts,
    pub plan: NormalizedPlan,
    pub relations: Vec<RelationStats>,
}
