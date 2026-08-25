use crate::{
    AffectedRowsTrigger, AnalysisInput, Config, Diagnostic, DiagnosticEvidence,
    DiagnosticThresholds, JoinGraph, PlanNode, PlanNodeKind, RelationOccurrence, RelationRef,
    Report, ReportStatus, ReportSummary, RuleId, Severity, StatementKind,
};

pub fn analyze(input: &AnalysisInput, config: &Config) -> Report {
    let mut diagnostics = Vec::new();

    if let Some(diagnostic) = missing_where_diagnostic(input, config) {
        diagnostics.push(diagnostic);
    }
    if let Some(diagnostic) = large_affected_rows_diagnostic(input, config) {
        diagnostics.push(diagnostic);
    }
    diagnostics.extend(large_sequential_scan_diagnostics(input, config));
    if let Some(diagnostic) = large_result_set_diagnostic(input, config) {
        diagnostics.push(diagnostic);
    }
    if let Some(diagnostic) = cartesian_join_diagnostic(input, config) {
        diagnostics.push(diagnostic);
    }

    diagnostics.sort_by(|left, right| {
        left.severity
            .cmp(&right.severity)
            .then(left.rule_id.cmp(&right.rule_id))
            .then_with(|| {
                evidence_relation(&left.evidence).cmp(&evidence_relation(&right.evidence))
            })
    });

    report(input.statement.kind, diagnostics)
}

fn missing_where_diagnostic(input: &AnalysisInput, config: &Config) -> Option<Diagnostic> {
    if input.statement.has_where {
        return None;
    }

    let (enabled, rule_id, title, message) = match input.statement.kind {
        StatementKind::Update => (
            config.rules.pgp001.enabled,
            RuleId::PGP001,
            "UPDATE without WHERE",
            "UPDATE target has no syntactic WHERE clause.",
        ),
        StatementKind::Delete => (
            config.rules.pgp002.enabled,
            RuleId::PGP002,
            "DELETE without WHERE",
            "DELETE target has no syntactic WHERE clause.",
        ),
        StatementKind::Select => return None,
    };

    if !enabled {
        return None;
    }

    Some(Diagnostic {
        rule_id,
        severity: Severity::Error,
        title: title.to_owned(),
        message: message.to_owned(),
        evidence: DiagnosticEvidence::MissingWhere {
            relation: target_relation(input)?.clone(),
            estimated_affected_rows: input.plan.estimated_affected_rows,
        },
        thresholds: None,
    })
}

fn large_affected_rows_diagnostic(input: &AnalysisInput, config: &Config) -> Option<Diagnostic> {
    if !config.rules.pgp101.enabled
        || !matches!(
            input.statement.kind,
            StatementKind::Update | StatementKind::Delete
        )
    {
        return None;
    }

    let estimated_affected_rows = input.plan.estimated_affected_rows?;
    let relation = target_relation(input)?;
    let estimated_relation_rows = input
        .relations
        .iter()
        .find(|stats| stats.relation == *relation)
        .and_then(|stats| stats.estimated_live_rows)
        .filter(|rows| *rows > 0.0);
    let estimated_relation_ratio =
        estimated_relation_rows.map(|rows| estimated_affected_rows / rows);

    let rule = &config.rules.pgp101;
    let mut triggered_by = Vec::new();
    if estimated_affected_rows >= rule.max_rows {
        triggered_by.push(AffectedRowsTrigger::AbsoluteRows);
    }
    if estimated_affected_rows >= rule.min_rows_for_ratio
        && estimated_relation_ratio.is_some_and(|ratio| ratio >= rule.max_table_ratio)
    {
        triggered_by.push(AffectedRowsTrigger::RelationRatio);
    }

    if triggered_by.is_empty() {
        return None;
    }

    Some(Diagnostic {
        rule_id: RuleId::PGP101,
        severity: Severity::Warning,
        title: "Large affected row set".to_owned(),
        message: "Estimated affected rows meet or exceed a configured PGP101 threshold.".to_owned(),
        evidence: DiagnosticEvidence::LargeAffectedRows {
            relation: relation.clone(),
            estimated_affected_rows,
            estimated_relation_rows,
            estimated_relation_ratio,
            triggered_by,
        },
        thresholds: Some(DiagnosticThresholds::LargeAffectedRows {
            max_rows: rule.max_rows,
            max_table_ratio: rule.max_table_ratio,
            min_rows_for_ratio: rule.min_rows_for_ratio,
        }),
    })
}

fn large_sequential_scan_diagnostics(input: &AnalysisInput, config: &Config) -> Vec<Diagnostic> {
    if !config.rules.pgp102.enabled {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    collect_large_sequential_scan_diagnostics(&input.plan.root, input, config, &mut diagnostics);
    diagnostics
}

fn collect_large_sequential_scan_diagnostics(
    node: &PlanNode,
    input: &AnalysisInput,
    config: &Config,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(diagnostic) = large_sequential_scan_diagnostic(node, input, config) {
        diagnostics.push(diagnostic);
    }

    for child in &node.children {
        collect_large_sequential_scan_diagnostics(child, input, config, diagnostics);
    }
}

fn large_sequential_scan_diagnostic(
    node: &PlanNode,
    input: &AnalysisInput,
    config: &Config,
) -> Option<Diagnostic> {
    if node.kind != PlanNodeKind::SeqScan {
        return None;
    }

    let relation = node.relation.as_ref()?;
    let estimated_scanned_rows = input
        .relations
        .iter()
        .find(|stats| stats.relation == *relation)
        .and_then(|stats| stats.estimated_live_rows)
        .filter(|rows| *rows > 0.0)?;
    let estimated_output_rows = node.estimated_rows;
    let estimated_output_ratio = estimated_output_rows / estimated_scanned_rows;
    let rule = &config.rules.pgp102;

    if !(estimated_scanned_rows >= rule.min_relation_rows
        && estimated_output_ratio <= rule.max_output_ratio)
    {
        return None;
    }

    Some(Diagnostic {
        rule_id: RuleId::PGP102,
        severity: Severity::Warning,
        title: "Large sequential scan".to_owned(),
        message: "Sequential scan meets the configured relation-size and output-ratio thresholds."
            .to_owned(),
        evidence: DiagnosticEvidence::LargeSequentialScan {
            relation: relation.clone(),
            alias: node.relation_alias.clone(),
            estimated_scanned_rows,
            estimated_output_rows,
            estimated_output_ratio,
        },
        thresholds: Some(DiagnosticThresholds::LargeSequentialScan {
            min_relation_rows: rule.min_relation_rows,
            max_output_ratio: rule.max_output_ratio,
        }),
    })
}

fn large_result_set_diagnostic(input: &AnalysisInput, config: &Config) -> Option<Diagnostic> {
    if !config.rules.pgp103.enabled || input.statement.kind != StatementKind::Select {
        return None;
    }

    let estimated_result_rows = input.plan.root.estimated_rows;
    let rule = &config.rules.pgp103;
    if estimated_result_rows < rule.max_result_rows {
        return None;
    }

    Some(Diagnostic {
        rule_id: RuleId::PGP103,
        severity: Severity::Warning,
        title: "Large estimated result set".to_owned(),
        message: "Estimated SELECT result rows meet or exceed the configured PGP103 threshold."
            .to_owned(),
        evidence: DiagnosticEvidence::LargeResultSet {
            estimated_result_rows,
        },
        thresholds: Some(DiagnosticThresholds::LargeResultSet {
            max_result_rows: rule.max_result_rows,
        }),
    })
}

fn cartesian_join_diagnostic(input: &AnalysisInput, config: &Config) -> Option<Diagnostic> {
    let graph = &input.statement.join_graph;
    if !config.rules.pgp104.enabled || graph.indeterminate || graph.relation_occurrences.len() < 2 {
        return None;
    }

    let disconnected_groups = connected_components(graph)?;
    if disconnected_groups.len() < 2 {
        return None;
    }

    let estimated_rows = match input.statement.kind {
        StatementKind::Select => Some(input.plan.root.estimated_rows),
        StatementKind::Update | StatementKind::Delete => input.plan.estimated_affected_rows,
    };

    Some(Diagnostic {
        rule_id: RuleId::PGP104,
        severity: Severity::Warning,
        title: "Possible Cartesian join".to_owned(),
        message: "Relation occurrences form multiple disconnected predicate components.".to_owned(),
        evidence: DiagnosticEvidence::CartesianJoin {
            disconnected_groups,
            estimated_rows,
        },
        thresholds: None,
    })
}

fn connected_components(graph: &JoinGraph) -> Option<Vec<Vec<RelationOccurrence>>> {
    let occurrence_count = graph.relation_occurrences.len();
    let mut adjacency = vec![Vec::new(); occurrence_count];

    for edge in &graph.edges {
        if edge.left >= occurrence_count || edge.right >= occurrence_count {
            return None;
        }
        if edge.left == edge.right {
            continue;
        }
        adjacency[edge.left].push(edge.right);
        adjacency[edge.right].push(edge.left);
    }

    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
        neighbors.dedup();
    }

    let mut visited = vec![false; occurrence_count];
    let mut components = Vec::new();

    for start in 0..occurrence_count {
        if visited[start] {
            continue;
        }

        visited[start] = true;
        let mut stack = vec![start];
        let mut component_indices = Vec::new();

        while let Some(current) = stack.pop() {
            component_indices.push(current);
            for &neighbor in adjacency[current].iter().rev() {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }

        component_indices.sort_unstable();
        components.push(
            component_indices
                .into_iter()
                .map(|index| graph.relation_occurrences[index].clone())
                .collect(),
        );
    }

    Some(components)
}

fn target_relation(input: &AnalysisInput) -> Option<&RelationRef> {
    input
        .statement
        .target_relation
        .as_ref()
        .or(input.plan.root.relation.as_ref())
}

fn evidence_relation(evidence: &DiagnosticEvidence) -> Option<&RelationRef> {
    match evidence {
        DiagnosticEvidence::MissingWhere { relation, .. }
        | DiagnosticEvidence::LargeAffectedRows { relation, .. }
        | DiagnosticEvidence::LargeSequentialScan { relation, .. } => Some(relation),
        DiagnosticEvidence::LargeResultSet { .. } | DiagnosticEvidence::CartesianJoin { .. } => {
            None
        }
    }
}

fn report(kind: StatementKind, diagnostics: Vec<Diagnostic>) -> Report {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count() as u32;
    let warnings = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .count() as u32;

    let status = if errors > 0 {
        ReportStatus::Errors
    } else if warnings > 0 {
        ReportStatus::Warnings
    } else {
        ReportStatus::Clean
    };

    let mut report = Report::clean(kind);
    report.status = status;
    report.summary = ReportSummary { errors, warnings };
    report.diagnostics = diagnostics;
    report
}
