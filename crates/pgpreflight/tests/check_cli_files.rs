use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Sandbox {
    path: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "pgpreflight-cli-files-{label}-{}-{id}",
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
        fs::write(&path, bytes).expect("write sandbox file");
        path
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_cli(current_dir: &Path, args: &[&str], database_url: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pgpreflight"));
    command
        .current_dir(current_dir)
        .args(args)
        .env_remove("PGPREFLIGHT_DATABASE_URL")
        .env_remove("DATABASE_URL");
    if let Some(database_url) = database_url {
        command.env("DATABASE_URL", database_url);
    }
    command.output().expect("run pgpreflight")
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

#[test]
fn sql_file_uses_database_url_fallback_and_returns_clean_json() {
    let Ok(database_url) = env::var("PGPREFLIGHT_TEST_DATABASE_URL") else {
        return;
    };
    let sandbox = Sandbox::new("success");
    let input = sandbox.write("query.sql", b"SELECT 1;");
    let output = run_cli(
        sandbox.path(),
        &[
            "check",
            input.to_str().expect("UTF-8 input path"),
            "--format",
            "json",
        ],
        Some(database_url.as_str()),
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(json_report(&output)["status"].as_str(), Some("clean"));
}

#[test]
fn missing_sql_file_is_a_structured_json_failure() {
    let sandbox = Sandbox::new("missing");
    let missing = sandbox.path().join("missing.sql");
    let output = run_cli(
        sandbox.path(),
        &[
            "check",
            missing.to_str().expect("UTF-8 input path"),
            "--format",
            "json",
        ],
        None,
    );

    assert_eq!(output.status.code(), Some(2));
    let report = json_report(&output);
    assert_eq!(report["status"].as_str(), Some("failed"));
    assert_eq!(report["failure"]["kind"].as_str(), Some("input_io"));
}
