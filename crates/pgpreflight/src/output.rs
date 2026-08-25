use std::io::{self, Write};

use clap::ValueEnum;
use pgpreflight_core::{
    FailureInfo, Report, ReportStatus, ReportSummary, Severity, StatementKind, StatementSummary,
    ToolInfo,
};

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum FailOn {
    Error,
    Warning,
}

pub(crate) struct CliFailure {
    kind: &'static str,
    message: &'static str,
    statement: Option<StatementKind>,
}

impl CliFailure {
    pub(crate) const fn new(kind: &'static str, message: &'static str) -> Self {
        Self {
            kind,
            message,
            statement: None,
        }
    }

    pub(crate) const fn for_statement(
        kind: &'static str,
        message: &'static str,
        statement: StatementKind,
    ) -> Self {
        Self {
            kind,
            message,
            statement: Some(statement),
        }
    }

    pub(crate) fn into_report(self) -> Report {
        Report {
            schema_version: 1,
            tool: ToolInfo {
                name: "pgpreflight".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            status: ReportStatus::Failed,
            statement: self.statement.map(|kind| StatementSummary { kind }),
            summary: ReportSummary {
                errors: 0,
                warnings: 0,
            },
            diagnostics: Vec::new(),
            failure: Some(FailureInfo {
                kind: self.kind.to_owned(),
                message: self.message.to_owned(),
            }),
        }
    }
}

pub(crate) fn exit_code(report: &Report, fail_on: FailOn) -> u8 {
    if report.status == ReportStatus::Failed {
        return 2;
    }

    let threshold_reached = match fail_on {
        FailOn::Error => report.summary.errors > 0,
        FailOn::Warning => report.summary.errors > 0 || report.summary.warnings > 0,
    };
    u8::from(threshold_reached)
}

pub(crate) fn write_report(format: OutputFormat, report: &Report) -> io::Result<()> {
    match format {
        OutputFormat::Json => write_json(report),
        OutputFormat::Text if report.status == ReportStatus::Failed => {
            let stderr = io::stderr();
            write_text_failure(&mut stderr.lock(), report)
        }
        OutputFormat::Text => {
            let stdout = io::stdout();
            write_text_report(&mut stdout.lock(), report)
        }
    }
}

fn write_json(report: &Report) -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, report).map_err(io::Error::other)?;
    writeln!(output)
}

fn write_text_failure(output: &mut impl Write, report: &Report) -> io::Result<()> {
    writeln!(output, "pgpreflight: failed")?;
    if let Some(failure) = &report.failure {
        writeln!(output, "error: {}", failure.message)?;
    }
    Ok(())
}

fn write_text_report(output: &mut impl Write, report: &Report) -> io::Result<()> {
    writeln!(output, "pgpreflight: {}", status_label(report.status))?;
    if let Some(statement) = &report.statement {
        writeln!(output, "statement: {}", statement_kind_label(statement.kind))?;
    }
    writeln!(
        output,
        "summary: {} errors, {} warnings",
        report.summary.errors, report.summary.warnings
    )?;

    for diagnostic in &report.diagnostics {
        writeln!(
            output,
            "{} {:?}: {}",
            severity_label(diagnostic.severity),
            diagnostic.rule_id,
            diagnostic.title
        )?;
        writeln!(output, "  {}", diagnostic.message)?;
    }

    Ok(())
}

const fn status_label(status: ReportStatus) -> &'static str {
    match status {
        ReportStatus::Clean => "clean",
        ReportStatus::Warnings => "warnings",
        ReportStatus::Errors => "errors",
        ReportStatus::Failed => "failed",
    }
}

const fn statement_kind_label(kind: StatementKind) -> &'static str {
    match kind {
        StatementKind::Select => "select",
        StatementKind::Update => "update",
        StatementKind::Delete => "delete",
    }
}

const fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

#[cfg(test)]
mod tests {
    use pgpreflight_core::{Report, ReportStatus, StatementKind};

    use super::{FailOn, exit_code};

    #[test]
    fn exit_code_honors_the_selected_diagnostic_threshold() {
        let mut report = Report::clean(StatementKind::Select);
        report.status = ReportStatus::Warnings;
        report.summary.warnings = 1;

        assert_eq!(exit_code(&report, FailOn::Error), 0);
        assert_eq!(exit_code(&report, FailOn::Warning), 1);
    }
}
