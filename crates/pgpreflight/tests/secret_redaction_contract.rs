use serde_json::Value;
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
const SECRET_URL: &str =
    "postgresql://user:credential-secret-marker@127.0.0.1:1/pgpreflight";

struct Sandbox {
    path: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "pgpreflight-secret-contract-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create sandbox");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_cli(current_dir: &Path, args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pgpreflight"))
        .current_dir(current_dir)
        .args(args)
        .env_remove("PGPREFLIGHT_DATABASE_URL")
        .env_remove("DATABASE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pgpreflight");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin)
        .expect("write stdin");
    child.wait_with_output().expect("wait for pgpreflight")
}

fn assert_exit_two(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_output_excludes(output: &Output, forbidden: &[&str]) {
    let rendered = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for value in forbidden {
        assert!(
            !rendered.contains(value),
            "stdout/stderr leaked {value:?}: {rendered}"
        );
    }
}

#[test]
fn parser_failures_do_not_leak_sql_credentials_or_raw_parser_errors_to_any_stream() {
    let sandbox = Sandbox::new("parser");
    let sql = b"SELECT 'literal-secret-marker' FROM";

    let text = run_cli(
        sandbox.path(),
        &["check", "-", "--database-url", SECRET_URL],
        sql,
    );
    assert_exit_two(&text);
    assert!(text.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&text.stderr),
        "pgpreflight: failed\nerror: SQL could not be parsed.\n"
    );

    let json = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--database-url",
            SECRET_URL,
            "--format",
            "json",
        ],
        sql,
    );
    assert_exit_two(&json);
    assert!(json.stderr.is_empty());
    let report: Value = serde_json::from_slice(&json.stdout).expect("parse JSON report");
    assert_eq!(report["failure"]["kind"], "sql_parse");
    assert_eq!(report["failure"]["message"], "SQL could not be parsed.");

    for output in [&text, &json] {
        assert_output_excludes(
            output,
            &[
                "literal-secret-marker",
                "credential-secret-marker",
                SECRET_URL,
                "ParserError",
                "Expected",
                "sql parser error",
            ],
        );
    }
}

#[test]
fn driver_failures_do_not_leak_sql_credentials_or_raw_driver_errors_to_any_stream() {
    let sandbox = Sandbox::new("driver");
    let sql = b"SELECT 'literal-secret-marker';";

    let text = run_cli(
        sandbox.path(),
        &["check", "-", "--database-url", SECRET_URL],
        sql,
    );
    assert_exit_two(&text);
    assert!(text.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&text.stderr),
        "pgpreflight: failed\nerror: database connection failed.\n"
    );

    let json = run_cli(
        sandbox.path(),
        &[
            "check",
            "-",
            "--database-url",
            SECRET_URL,
            "--format",
            "json",
        ],
        sql,
    );
    assert_exit_two(&json);
    assert!(json.stderr.is_empty());
    let report: Value = serde_json::from_slice(&json.stdout).expect("parse JSON report");
    assert_eq!(report["failure"]["kind"], "database_connection");
    assert_eq!(report["failure"]["message"], "database connection failed.");

    for output in [&text, &json] {
        assert_output_excludes(
            output,
            &[
                "literal-secret-marker",
                "credential-secret-marker",
                SECRET_URL,
                "Connection refused",
                "os error",
                "tcp connect error",
                "No connection could be made",
            ],
        );
    }
}
