use crate::{
    AffectedRowsTrigger, AnalysisInput, Config, Diagnostic, DiagnosticEvidence,
    DiagnosticThresholds, RelationRef, Report, ReportStatus, ReportSummary, RuleId, Severity,
    StatementKind,
};

pub fn analyze(input: &AnalysisInput, config: &Config) -> Report {
    let mut diagnostics = Vec::new();

    if let Some(diagnostic) = missing_where_diagnostic(input, config) {
        diagnostics.push(diagnostic);
    }
    if let Some(diagnostic) = large_affected_rows_diagnostic(input, config) {
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
