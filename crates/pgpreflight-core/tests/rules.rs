use pgpreflight_core::{
    AffectedRowsTrigger, AnalysisInput, Config, DiagnosticEvidence, JoinGraph, NormalizedPlan,
    PlanNode, PlanNodeKind, RelationRef, RelationStats, ReportStatus, RuleId, Severity,
    StatementFacts, StatementKind, analyze,
};

fn target_relation() -> RelationRef {
    RelationRef::new("public", "widgets")
}

fn plan_node(relation: Option<RelationRef>) -> PlanNode {
    PlanNode {
        kind: PlanNodeKind::ModifyTable,
        estimated_rows: 0.0,
        startup_cost: 0.0,
        total_cost: 1.0,
        relation,
        relation_alias: None,
        children: Vec::new(),
    }
}

fn analysis_input(
    kind: StatementKind,
    has_where: bool,
    estimated_affected_rows: Option<f64>,
    relation_rows: Option<Option<f64>>,
) -> AnalysisInput {
    let relation = target_relation();
    AnalysisInput {
        statement: StatementFacts {
            kind,
            target_relation: Some(relation.clone()),
            has_where,
            has_returning: false,
            join_graph: JoinGraph::default(),
        },
        plan: NormalizedPlan {
            root: plan_node(Some(relation.clone())),
            estimated_affected_rows,
        },
        relations: relation_rows
            .map(|estimated_live_rows| {
                vec![RelationStats {
                    relation,
                    estimated_live_rows,
                    pages: None,
                }]
            })
            .unwrap_or_default(),
    }
}

fn large_affected_config() -> Config {
    let mut config = Config::default();
    config.rules.pgp101.max_rows = 100.0;
    config.rules.pgp101.max_table_ratio = 0.25;
    config.rules.pgp101.min_rows_for_ratio = 20.0;
    config
}

#[test]
fn missing_where_rules_do_not_require_planner_evidence() {
    for (kind, expected_rule) in [
        (StatementKind::Update, RuleId::PGP001),
        (StatementKind::Delete, RuleId::PGP002),
    ] {
        let mut input = analysis_input(kind, false, None, None);
        input.plan.root = plan_node(None);

        let report = analyze(&input, &Config::default());

        assert_eq!(report.status, ReportStatus::Errors);
        assert_eq!(report.summary.errors, 1);
        assert_eq!(report.summary.warnings, 0);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].rule_id, expected_rule);
        assert_eq!(report.diagnostics[0].severity, Severity::Error);
        assert_eq!(
            report.diagnostics[0].evidence,
            DiagnosticEvidence::MissingWhere {
                relation: target_relation(),
                estimated_affected_rows: None,
            }
        );
    }
}

#[test]
fn syntactically_present_where_skips_missing_where_rules() {
    for kind in [StatementKind::Update, StatementKind::Delete] {
        let report = analyze(&analysis_input(kind, true, None, None), &Config::default());

        assert_eq!(report.status, ReportStatus::Clean);
        assert!(report.diagnostics.is_empty());
    }
}

#[test]
fn pgp101_absolute_rows_triggers_at_exact_boundary() {
    let config = large_affected_config();

    let below = analyze(
        &analysis_input(StatementKind::Update, true, Some(99.0), None),
        &config,
    );
    assert!(below.diagnostics.is_empty());

    let at_boundary = analyze(
        &analysis_input(StatementKind::Update, true, Some(100.0), None),
        &config,
    );
    assert_eq!(at_boundary.diagnostics.len(), 1);
    assert_eq!(at_boundary.diagnostics[0].rule_id, RuleId::PGP101);
    assert_eq!(
        at_boundary.diagnostics[0].evidence,
        DiagnosticEvidence::LargeAffectedRows {
            relation: target_relation(),
            estimated_affected_rows: 100.0,
            estimated_relation_rows: None,
            estimated_relation_ratio: None,
            triggered_by: vec![AffectedRowsTrigger::AbsoluteRows],
        }
    );
}

#[test]
fn pgp101_ratio_triggers_at_exact_ratio_boundary() {
    let config = large_affected_config();

    let below = analyze(
        &analysis_input(StatementKind::Update, true, Some(24.0), Some(Some(100.0))),
        &config,
    );
    assert!(below.diagnostics.is_empty());

    let at_boundary = analyze(
        &analysis_input(StatementKind::Update, true, Some(25.0), Some(Some(100.0))),
        &config,
    );
    assert_eq!(at_boundary.diagnostics.len(), 1);
    assert_eq!(
        at_boundary.diagnostics[0].evidence,
        DiagnosticEvidence::LargeAffectedRows {
            relation: target_relation(),
            estimated_affected_rows: 25.0,
            estimated_relation_rows: Some(100.0),
            estimated_relation_ratio: Some(0.25),
            triggered_by: vec![AffectedRowsTrigger::RelationRatio],
        }
    );
}

#[test]
fn pgp101_ratio_requires_min_rows_and_includes_exact_minimum() {
    let config = large_affected_config();

    let below_minimum = analyze(
        &analysis_input(StatementKind::Delete, true, Some(19.0), Some(Some(76.0))),
        &config,
    );
    assert!(below_minimum.diagnostics.is_empty());

    let at_minimum = analyze(
        &analysis_input(StatementKind::Delete, true, Some(20.0), Some(Some(80.0))),
        &config,
    );
    assert_eq!(at_minimum.diagnostics.len(), 1);
    assert_eq!(at_minimum.diagnostics[0].rule_id, RuleId::PGP101);
}

#[test]
fn pgp101_missing_or_non_positive_stats_only_evaluate_absolute_threshold() {
    let config = large_affected_config();

    for relation_rows in [None, Some(None), Some(Some(0.0))] {
        let report = analyze(
            &analysis_input(StatementKind::Update, true, Some(50.0), relation_rows),
            &config,
        );
        assert!(report.diagnostics.is_empty());
    }

    let absolute = analyze(
        &analysis_input(StatementKind::Update, true, Some(100.0), Some(None)),
        &config,
    );
    assert_eq!(absolute.diagnostics.len(), 1);
    assert_eq!(absolute.diagnostics[0].rule_id, RuleId::PGP101);
}

#[test]
fn pgp101_records_both_triggers_in_stable_order() {
    let config = large_affected_config();
    let report = analyze(
        &analysis_input(StatementKind::Update, true, Some(100.0), Some(Some(200.0))),
        &config,
    );

    let DiagnosticEvidence::LargeAffectedRows { triggered_by, .. } =
        &report.diagnostics[0].evidence
    else {
        panic!("expected large affected rows evidence");
    };
    assert_eq!(
        triggered_by,
        &vec![
            AffectedRowsTrigger::AbsoluteRows,
            AffectedRowsTrigger::RelationRatio,
        ]
    );
}

#[test]
fn diagnostics_and_summary_are_deterministic() {
    let config = large_affected_config();
    let report = analyze(
        &analysis_input(StatementKind::Update, false, Some(100.0), Some(Some(200.0))),
        &config,
    );

    assert_eq!(report.status, ReportStatus::Errors);
    assert_eq!(report.summary.errors, 1);
    assert_eq!(report.summary.warnings, 1);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.severity, diagnostic.rule_id))
            .collect::<Vec<_>>(),
        vec![
            (Severity::Error, RuleId::PGP001),
            (Severity::Warning, RuleId::PGP101),
        ]
    );
}

#[test]
fn disabled_rules_are_not_emitted() {
    let mut config = large_affected_config();
    config.rules.pgp001.enabled = false;
    config.rules.pgp101.enabled = false;

    let report = analyze(
        &analysis_input(StatementKind::Update, false, Some(100.0), Some(Some(200.0))),
        &config,
    );

    assert_eq!(report.status, ReportStatus::Clean);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn normalized_plan_relation_resolves_unqualified_target_for_diagnostics() {
    let mut input = analysis_input(StatementKind::Delete, false, Some(100.0), Some(Some(200.0)));
    input.statement.target_relation = None;

    let report = analyze(&input, &large_affected_config());

    assert_eq!(report.diagnostics.len(), 2);
    for diagnostic in &report.diagnostics {
        match &diagnostic.evidence {
            DiagnosticEvidence::MissingWhere { relation, .. }
            | DiagnosticEvidence::LargeAffectedRows { relation, .. } => {
                assert_eq!(relation, &target_relation());
            }
            other => panic!("unexpected evidence: {other:?}"),
        }
    }
}
