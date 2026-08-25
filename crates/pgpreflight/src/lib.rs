#![forbid(unsafe_code)]

mod input;
mod output;
mod settings;

use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};
use input::InputFailure;
use output::{CliFailure, FailOn, OutputFormat};
use pgpreflight_core::{Report, StatementKind, analyze};
use pgpreflight_postgres::{CheckError, PlanningError, SafeModePlanner, parse_and_validate};
use settings::{ConfigFailure, DatabaseUrlFailure};

#[derive(Parser)]
#[command(
    name = "pgpreflight",
    version,
    about = "Conservatively check PostgreSQL SQL before execution"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check exactly one SQL statement without executing it.
    Check(CheckArgs),
}

#[derive(Args)]
struct CheckArgs {
    /// SQL file path, or - to read from standard input.
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    /// Lowest diagnostic severity that produces exit code 1.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    fail_on: FailOn,

    /// Explicit configuration file path.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// PostgreSQL connection URL.
    #[arg(long, value_name = "URL")]
    database_url: Option<String>,
}

pub async fn run() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check(args) => run_check(args).await,
    }
}

async fn run_check(args: CheckArgs) -> ExitCode {
    let format = args.format;
    let fail_on = args.fail_on;
    let report = match check(args).await {
        Ok(report) => report,
        Err(failure) => failure.into_report(),
    };
    let exit_code = output::exit_code(&report, fail_on);

    if output::write_report(format, &report).is_err() {
        return ExitCode::from(2);
    }

    ExitCode::from(exit_code)
}

async fn check(args: CheckArgs) -> Result<Report, CliFailure> {
    let sql = input::read_sql(&args.input).map_err(map_input_failure)?;
    let statement = parse_and_validate(&sql).map_err(map_check_failure)?;
    drop(sql);

    let statement_kind = statement.facts().kind;
    let config = settings::load_config(args.config.as_deref())
        .map_err(|failure| map_config_failure(failure, statement_kind))?;
    let database_url = settings::resolve_database_url(args.database_url)
        .map_err(|failure| map_database_url_failure(failure, statement_kind))?;

    let mut planner = SafeModePlanner::connect(&database_url)
        .await
        .map_err(|failure| map_planning_failure(failure, statement_kind))?;
    drop(database_url);
    let planned = planner
        .plan(&statement, &config.postgres)
        .await
        .map_err(|failure| map_planning_failure(failure, statement_kind))?;

    Ok(analyze(planned.analysis_input(), &config))
}

const fn map_input_failure(failure: InputFailure) -> CliFailure {
    match failure {
        InputFailure::Io => CliFailure::new("input_io", "SQL input could not be read."),
        InputFailure::NotUtf8 => {
            CliFailure::new("input_not_utf8", "SQL input must be valid UTF-8.")
        }
        InputFailure::Empty => {
            CliFailure::new("empty_input", "exactly one SQL statement is required.")
        }
    }
}

const fn map_check_failure(failure: CheckError) -> CliFailure {
    match failure {
        CheckError::SqlParse => CliFailure::new("sql_parse", "SQL could not be parsed."),
        CheckError::MultipleStatements => CliFailure::new(
            "multiple_statements",
            "exactly one SQL statement is required.",
        ),
        CheckError::UnsupportedStatement => {
            CliFailure::new("unsupported_statement", "statement type is not supported.")
        }
        CheckError::UnsafeConstruct { .. } => CliFailure::new(
            "unsafe_sql",
            "SQL contains a construct that cannot be checked safely.",
        ),
    }
}

const fn map_config_failure(failure: ConfigFailure, statement: StatementKind) -> CliFailure {
    match failure {
        ConfigFailure::Io => CliFailure::for_statement(
            "config_io",
            "configuration file could not be read.",
            statement,
        ),
        ConfigFailure::Parse => {
            CliFailure::for_statement("config_parse", "configuration file is invalid.", statement)
        }
    }
}

const fn map_database_url_failure(
    failure: DatabaseUrlFailure,
    statement: StatementKind,
) -> CliFailure {
    match failure {
        DatabaseUrlFailure::Missing => CliFailure::for_statement(
            "database_url_missing",
            "database URL is required.",
            statement,
        ),
        DatabaseUrlFailure::Invalid => CliFailure::for_statement(
            "database_url_invalid",
            "database URL is invalid.",
            statement,
        ),
    }
}

const fn map_planning_failure(failure: PlanningError, statement: StatementKind) -> CliFailure {
    let (kind, message) = match failure {
        PlanningError::Connection => ("database_connection", "database connection failed."),
        PlanningError::Transaction => ("database_transaction", "safe planning transaction failed."),
        PlanningError::Configuration => (
            "database_configuration",
            "safe planning configuration failed.",
        ),
        PlanningError::Timeout => ("database_timeout", "database planning timed out."),
        PlanningError::Planning => ("database_planning", "database planning failed."),
        PlanningError::InvalidPlan => ("invalid_plan", "database returned an invalid plan."),
        PlanningError::Catalog => ("database_catalog", "database catalog lookup failed."),
        PlanningError::Rollback => ("database_rollback", "safe planning rollback failed."),
    };
    CliFailure::for_statement(kind, message, statement)
}
