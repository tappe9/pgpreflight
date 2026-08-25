use pgpreflight_core::{
    AnalysisInput, Config, DiagnosticEvidence, DiagnosticThresholds, JoinGraph, NormalizedPlan,
    PlanNode, PlanNodeKind, RelationRef, RelationStats, ReportStatus, RuleId, StatementFacts,
    StatementKind, analyze,
};

fn relation(name: &str) -> RelationRef {
    RelationRef::new("public", name)
}

fn plan_node(
    kind: PlanNodeKind,
    estimated_rows: f64,
    relation: Option<RelationRef>,
    alias: Option<&str>,
    children: Vec<PlanNode>,
) -> PlanNode {
    PlanNode {
        kind,
        estimated_rows,
        startup_cost: 0.0,
        total_cost: 1.0,
        relation,
        relation_alias: alias.map(str::to_owned),
        children,
    }
}

fn seq_scan(relation: RelationRef, alias: &str, estimated_rows: f64) -> PlanNode {
    plan_node(
        PlanNodeKind::SeqScan,
        estimated_rows,
        Some(relation),
        Some(alias),
        Vec::new(),
    )
}

fn relation_stats(relation: RelationRef, estimated_live_rows: Option<f64>) -> RelationStats {
    RelationStats {
        relation,
        estimated_live_rows,
        pages: None,
    }
}

fn analysis_input(
    kind: StatementKind,
    has_returning: bool,
    root: PlanNode,
    relations: Vec<RelationStats>,
) -> AnalysisInput {
    let target_relation = root.relation.clone();
    AnalysisInput {
        statement: StatementFacts {
            kind,
            target_relation,
            has_where: true,
            has_returning,
            join_graph: JoinGraph::default(),
        },
        plan: NormalizedPlan {
            root,
            estimated_affected_rows: None,
        },
        relations,
    }
}

fn scan_result_config() -> Config {
    let mut config = Config::default();
    config.rules.pgp001.enabled = false;
    config.rules.pgp002.enabled = false;
    config.rules.pgp101.enabled = false;
    config.rules.pgp104.enabled = false;
    config.rules.pgp102.min_relation_rows = 100.0;
    config.rules.pgp102.max_output_ratio = 0.25;
    config.rules.pgp103.max_result_rows = 1_000.0;
    config
}

#[test]
fn pgp102_requires_both_thresholds_and_includes_exact_boundaries() {
    let relation = relation("widgets");
    let config = scan_result_config();

    let below_relation_size = analyze(
        &analysis_input(
            StatementKind::Select,
            false,
            seq_scan(relation.clone(), "w", 24.75),
            vec![relation_stats(relation.clone(), Some(99.0))],
        ),
        &config,
    );
    assert!(below_relation_size.diagnostics.is_empty());

    let above_output_ratio = analyze(
        &analysis_input(
            StatementKind::Select,
            false,
            seq_scan(relation.clone(), "w", 26.0),
            vec![relation_stats(relation.clone(), Some(100.0))],
        ),
        &config,
    );
    assert!(above_output_ratio.diagnostics.is_empty());

    let at_both_boundaries = analyze(
        &analysis_input(
            StatementKind::Select,
            false,
            seq_scan(relation.clone(), "w", 25.0),
            vec![relation_stats(relation.clone(), Some(100.0))],
        ),
        &config,
    );

    assert_eq!(at_both_boundaries.diagnostics.len(), 1);
    assert_eq!(at_both_boundaries.diagnostics[0].rule_id, RuleId::PGP102);
    assert_eq!(
        at_both_boundaries.diagnostics[0].evidence,
        DiagnosticEvidence::LargeSequentialScan {
            relation,
            alias: Some("w".to_owned()),
            estimated_scanned_rows: 100.0,
            estimated_output_rows: 25.0,
            estimated_output_ratio: 0.25,
        }
    );
    assert_eq!(
        at_both_boundaries.diagnostics[0].thresholds,
        Some(DiagnosticThresholds::LargeSequentialScan {
            min_relation_rows: 100.0,
            max_output_ratio: 0.25,
        })
    );
}

#[test]
fn pgp102_skips_missing_non_positive_stats_and_non_seq_scans() {
    let relation = relation("widgets");
    let config = scan_result_config();

    for relations in [
        Vec::new(),
        vec![relation_stats(relation.clone(), None)],
        vec![relation_stats(relation.clone(), Some(0.0))],
        vec![relation_stats(relation.clone(), Some(-1.0))],
    ] {
        let report = analyze(
            &analysis_input(
                StatementKind::Select,
                false,
                seq_scan(relation.clone(), "w", 1.0),
                relations,
            ),
            &config,
        );
        assert!(report.diagnostics.is_empty());
    }

    let index_scan = plan_node(
        PlanNodeKind::IndexScan,
        1.0,
        Some(relation.clone()),
        Some("w"),
        Vec::new(),
    );
    let report = analyze(
        &analysis_input(
            StatementKind::Select,
            false,
            index_scan,
            vec![relation_stats(relation, Some(100.0))],
        ),
        &config,
    );
    assert!(report.diagnostics.is_empty());
}

#[test]
fn pgp102_evaluates_self_join_scans_independently_in_plan_order() {
    let relation = relation("widgets");
    let root = plan_node(
        PlanNodeKind::NestedLoop,
        10.0,
        None,
        None,
        vec![
            seq_scan(relation.clone(), "left_widgets", 10.0),
            seq_scan(relation.clone(), "right_widgets", 20.0),
        ],
    );
    let report = analyze(
        &analysis_input(
            StatementKind::Select,
            false,
            root,
            vec![relation_stats(relation, Some(100.0))],
        ),
        &scan_result_config(),
    );

    assert_eq!(report.diagnostics.len(), 2);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|diagnostic| match &diagnostic.evidence {
                DiagnosticEvidence::LargeSequentialScan {
                    alias,
                    estimated_output_rows,
                    ..
                } => (alias.as_deref(), *estimated_output_rows),
                other => panic!("unexpected evidence: {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![(Some("left_widgets"), 10.0), (Some("right_widgets"), 20.0)]
    );
}

#[test]
fn pgp103_uses_select_root_rows_and_respects_limit() {
    let relation = relation("widgets");
    let mut config = scan_result_config();
    config.rules.pgp103.max_result_rows = 100.0;

    let below_boundary = plan_node(
        PlanNodeKind::Limit,
        99.0,
        None,
        None,
        vec![seq_scan(relation.clone(), "w", 10_000.0)],
    );
    let below_report = analyze(
        &analysis_input(StatementKind::Select, false, below_boundary, Vec::new()),
        &config,
    );
    assert!(below_report.diagnostics.is_empty());

    let at_boundary = plan_node(
        PlanNodeKind::Limit,
        100.0,
        None,
        None,
        vec![seq_scan(relation, "w", 10_000.0)],
    );
    let at_boundary_report = analyze(
        &analysis_input(StatementKind::Select, false, at_boundary, Vec::new()),
        &config,
    );

    assert_eq!(at_boundary_report.diagnostics.len(), 1);
    assert_eq!(at_boundary_report.diagnostics[0].rule_id, RuleId::PGP103);
    assert_eq!(
        at_boundary_report.diagnostics[0].evidence,
        DiagnosticEvidence::LargeResultSet {
            estimated_result_rows: 100.0,
        }
    );
    assert_eq!(
        at_boundary_report.diagnostics[0].thresholds,
        Some(DiagnosticThresholds::LargeResultSet {
            max_result_rows: 100.0,
        })
    );
}

#[test]
fn pgp103_skips_update_and_delete_returning() {
    let relation = relation("widgets");
    let config = scan_result_config();

    for kind in [StatementKind::Update, StatementKind::Delete] {
        let root = plan_node(
            PlanNodeKind::ModifyTable,
            1_000_000.0,
            Some(relation.clone()),
            Some("widgets"),
            Vec::new(),
        );
        let report = analyze(
            &analysis_input(kind, true, root, Vec::new()),
            &config,
        );

        assert_eq!(report.status, ReportStatus::Clean);
        assert!(report.diagnostics.is_empty());
    }
}

#[test]
fn scan_and_result_diagnostics_are_ordered_and_counted_deterministically() {
    let alpha = relation("alpha");
    let zeta = relation("zeta");
    let root = plan_node(
        PlanNodeKind::NestedLoop,
        1_000.0,
        None,
        None,
        vec![
            seq_scan(zeta.clone(), "z", 10.0),
            seq_scan(alpha.clone(), "a", 10.0),
        ],
    );
    let report = analyze(
        &analysis_input(
            StatementKind::Select,
            false,
            root,
            vec![
                relation_stats(zeta, Some(100.0)),
                relation_stats(alpha, Some(100.0)),
            ],
        ),
        &scan_result_config(),
    );

    assert_eq!(report.status, ReportStatus::Warnings);
    assert_eq!(report.summary.errors, 0);
    assert_eq!(report.summary.warnings, 3);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|diagnostic| {
                let relation_name = match &diagnostic.evidence {
                    DiagnosticEvidence::LargeSequentialScan { relation, .. } => {
                        Some(relation.name.as_str())
                    }
                    DiagnosticEvidence::LargeResultSet { .. } => None,
                    other => panic!("unexpected evidence: {other:?}"),
                };
                (diagnostic.rule_id, relation_name)
            })
            .collect::<Vec<_>>(),
        vec![
            (RuleId::PGP102, Some("alpha")),
            (RuleId::PGP102, Some("zeta")),
            (RuleId::PGP103, None),
        ]
    );
}

#[test]
fn disabled_scan_and_result_rules_are_not_emitted() {
    let relation = relation("widgets");
    let root = plan_node(
        PlanNodeKind::Limit,
        1_000.0,
        None,
        None,
        vec![seq_scan(relation.clone(), "w", 10.0)],
    );
    let mut config = scan_result_config();
    config.rules.pgp102.enabled = false;
    config.rules.pgp103.enabled = false;

    let report = analyze(
        &analysis_input(
            StatementKind::Select,
            false,
            root,
            vec![relation_stats(relation, Some(100.0))],
        ),
        &config,
    );

    assert_eq!(report.status, ReportStatus::Clean);
    assert!(report.diagnostics.is_empty());
}
