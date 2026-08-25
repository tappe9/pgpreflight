use serde_json::Value;
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
use tokio_postgres::NoTls;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
const DATABASE_ENV_VARS: [&str; 2] = ["PGPREFLIGHT_DATABASE_URL", "DATABASE_URL"];

struct Sandbox {
    path: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "pgpreflight-cli-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create sandbox");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(&path, bytes).expect("write sandbox file");
        path
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_cli(
    current_dir: &Path,
    args: &[&str],
    stdin: &[u8],
    environment: &[(&str, &str)],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pgpreflight"));
    command
        .current_dir(current_dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for key in DATABASE_ENV_VARS {
        command.env_remove(key);
    }
    for (key, value) in environment {
        command.env(key, value);
    }

    let mut child = command.spawn().expect("spawn pgpreflight");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin)
        .expect("write stdin");
    child.wait_with_output().expect("wait for pgpreflight")
}

fn assert_exit(output: &Output, expected: i32) {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn json_report(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "JSON stderr was not empty: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout was not exactly one JSON object: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn failure_kind(report: &Value) -> Option<&str> {
    report
        .get("failure")
        .and_then(|failure| failure.get("kind"))
        .and_then(Value::as_str)
}

fn test_database_url() -> Option<String> {
    env::var("PGPREFLIGHT_TEST_DATABASE_URL").ok()
}

fn execute_sql(database_url: &str, sql: &str) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    runtime.block_on(async {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .expect("connect test database");
        tokio::spawn(async move {
            let _ = connection.await;
        });
        client.batch_execute(sql).await.expect("execute test SQL");
    });
}

fn query_bool(database_url: &str, sql: &str) -> bool {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    runtime.block_on(async {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .expect("connect test database");
        tokio::spawn(async move {
            let _ = connection.await;
        });
        client
            .query_one(sql, &[])
            .await
            .expect("query test database")
            .get(0)
    })
}

#[test]
fn json_stdin_with_bom_reaches_database_resolution() {
    let sandbox = Sandbox::new("bom");
    let output = run_cli(
        sandbox.path(),
        &["check", "-", "--format", "json"],
        b"\xef\xbb\xbfSELECT 1;",
        &[],
    );

    assert_exit(&output, 2);
    let report = json_report(&output);
    assert_eq!(failure_kind(&report), Some("database_url_missing"));
}

#[test]
fn json_empty_and_comment_only_inputs_are_structured_failures() {
    let sandbox = Sandbox::new("empty");
    for input in [
        b"".as_slice(),
        b" -- line only\n /* outer /* nested */ comment */ ; \n".as_slice(),
    ] {
        let output = run_cli(
            sandbox.path(),
            &["check", "-", "--format", "json"],
            input,
            &[],
        );

        assert_exit(&output, 2);
        let report = json_report(&output);
        assert_eq!(failure_kind(&report), Some("empty_input"));
    }
}

#[test]
fn json_invalid_utf8_is_a_structured_failure() {
    let sandbox = Sandbox::new("utf8");
    let output = run_cli(
        sandbox.path(),
        &["check", "-", "--format", "json"],
        &[0xff, 0xfe],
        &[],
    );

    assert_exit(&output, 2);
    let report = json_report(&output);
    assert_eq!(failure_kind(&report), Some("input_not_utf8"));
}

#[test]
fn json_multiple_statements_are_a_structured_failure() {
    let sandbox = Sandbox::new("multiple");
    let output = run_cli(
        sandbox.path(),
        &["check", "-", "--format", "json"],
        b"SELECT 1; SELECT 2;",
        &[],
    );

    assert_exit(&output, 2);
    let report = json_report(&output);
    assert_eq!(failure_kind(&report), Some("multiple_statements"));
}

#[test]
fn nearest_config_is_used_without_merging_parent_files() {
    let sandbox = Sandbox::new("config-nearest");
    sandbox.write(
        "pgpreflight.toml",
        b"version = 1\nunexpected_parent_field = true\n",
    );
    sandbox.write("child/pgpreflight.toml", b"version = 1\n");
    let child = sandbox.path().join("child");

    let output = run_cli(
        &child,
        &["check", "-", "--format", "json"],
        b"SELECT 1;",
        &[],
    );

    assert_exit(&output, 2);
    let report = json_report(&output);
    assert_eq!(failure_kind(&report), Some("database_url_missing"));
}

#[test]
fn explicit_config_overrides_discovery() {
    let sandbox = Sandbox::new("config-explicit");
    sandbox.write("pgpreflight.toml", b"version = 1\n");
    let explicit = sandbox.write(
        "configs/invalid.toml",
        b"version = 1\nunexpected_explicit_field = true\n",
    );

    let output = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--format",
            "json",
            "--config",
            explicit.to_str().expect("UTF-8 path"),
        ],
        b"SELECT 1;",
        &[],
    );

    assert_exit(&output, 2);
    let report = json_report(&output);
    assert_eq!(failure_kind(&report), Some("config_parse"));
}

#[test]
fn database_url_precedence_prefers_cli_then_pgpreflight_environment() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let sandbox = Sandbox::new("url-precedence");
    let invalid_url = "postgresql://precedence-marker@127.0.0.1:1/invalid";

    let explicit_output = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--format",
            "json",
            "--database-url",
            database_url.as_str(),
        ],
        b"SELECT 1;",
        &[
            ("PGPREFLIGHT_DATABASE_URL", invalid_url),
            ("DATABASE_URL", invalid_url),
        ],
    );
    assert_exit(&explicit_output, 0);
    assert_eq!(
        json_report(&explicit_output)["status"].as_str(),
        Some("clean")
    );

    let pgpreflight_env_output = run_cli(
        sandbox.path(),
        &["check", "-", "--format", "json"],
        b"SELECT 1;",
        &[
            ("PGPREFLIGHT_DATABASE_URL", database_url.as_str()),
            ("DATABASE_URL", invalid_url),
        ],
    );
    assert_exit(&pgpreflight_env_output, 0);
    assert_eq!(
        json_report(&pgpreflight_env_output)["status"].as_str(),
        Some("clean")
    );
}

#[test]
fn json_parser_failure_redacts_sql_literals_and_credentials() {
    let sandbox = Sandbox::new("parser-redaction");
    let secret_url = "postgresql://user:credential-marker@127.0.0.1:1/database";
    let output = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--format",
            "json",
            "--database-url",
            secret_url,
        ],
        b"SELECT 'literal-marker' FROM;",
        &[],
    );

    assert_exit(&output, 2);
    let report = json_report(&output);
    assert_eq!(failure_kind(&report), Some("sql_parse"));
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert!(!rendered.contains("literal-marker"));
    assert!(!rendered.contains("credential-marker"));
    assert!(!rendered.contains(secret_url));
}

#[test]
fn json_connection_failure_redacts_sql_literals_and_credentials() {
    let sandbox = Sandbox::new("connection-redaction");
    let secret_url = "postgresql://user:credential-marker@127.0.0.1:1/database";
    let output = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--format",
            "json",
            "--database-url",
            secret_url,
        ],
        b"SELECT 'literal-marker';",
        &[],
    );

    assert_exit(&output, 2);
    let report = json_report(&output);
    assert_eq!(failure_kind(&report), Some("database_connection"));
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert!(!rendered.contains("literal-marker"));
    assert!(!rendered.contains("credential-marker"));
    assert!(!rendered.contains(secret_url));
}

#[test]
fn fail_on_warning_changes_only_the_diagnostic_exit_status() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let sandbox = Sandbox::new("fail-on");
    let sql =
        b"SELECT * FROM pg_catalog.pg_class AS c CROSS JOIN pg_catalog.pg_namespace AS n LIMIT 1;";

    let error_threshold = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--format",
            "json",
            "--database-url",
            database_url.as_str(),
            "--fail-on",
            "error",
        ],
        sql,
        &[],
    );
    assert_exit(&error_threshold, 0);
    let report = json_report(&error_threshold);
    assert_eq!(report["status"].as_str(), Some("warnings"));
    assert!(report["summary"]["warnings"].as_u64().unwrap_or(0) >= 1);

    let warning_threshold = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--format",
            "json",
            "--database-url",
            database_url.as_str(),
            "--fail-on",
            "warning",
        ],
        sql,
        &[],
    );
    assert_exit(&warning_threshold, 1);
    assert_eq!(
        json_report(&warning_threshold)["status"].as_str(),
        Some("warnings")
    );
}

#[test]
fn connected_update_reports_an_error_without_modifying_the_table() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let sandbox = Sandbox::new("connected-update");
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let table = format!("pgpreflight_cli_{}_{}", std::process::id(), id);
    execute_sql(
        database_url.as_str(),
        &format!(
            "CREATE TABLE public.{table} (id integer PRIMARY KEY, active boolean NOT NULL); \
             INSERT INTO public.{table} VALUES (1, true);"
        ),
    );

    let sql = format!("UPDATE public.{table} SET active = false;");
    let output = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--format",
            "json",
            "--database-url",
            database_url.as_str(),
        ],
        sql.as_bytes(),
        &[],
    );

    assert_exit(&output, 1);
    let report = json_report(&output);
    assert_eq!(report["status"].as_str(), Some("errors"));
    assert_eq!(report["diagnostics"][0]["rule_id"].as_str(), Some("PGP001"));
    assert!(query_bool(
        database_url.as_str(),
        &format!("SELECT active FROM public.{table} WHERE id = 1")
    ));

    execute_sql(database_url.as_str(), &format!("DROP TABLE public.{table}"));
}

#[test]
fn text_tool_failure_uses_stderr_and_exit_two() {
    let sandbox = Sandbox::new("text-failure");
    let output = run_cli(sandbox.path(), &["check", "-"], b"SELECT 1;", &[]);

    assert_exit(&output, 2);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("database URL is required"));
    assert!(!stderr.contains("SELECT 1"));
}

#[test]
fn text_clean_report_uses_stdout() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let sandbox = Sandbox::new("text-clean");
    let output = run_cli(
        sandbox.path(),
        &["check", "-", "--database-url", database_url.as_str()],
        b"SELECT 1;",
        &[],
    );

    assert_exit(&output, 0);
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains("pgpreflight: clean"));
}
