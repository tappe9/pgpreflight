use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Sandbox {
    path: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "pgpreflight-cli-text-redaction-{}-{id}",
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

#[test]
fn text_connection_failure_redacts_sql_literals_credentials_and_driver_details() {
    let sandbox = Sandbox::new();
    let secret_url = "postgresql://user:credential-marker@127.0.0.1:1/database";
    let output = Command::new(env!("CARGO_BIN_EXE_pgpreflight"))
        .current_dir(sandbox.path())
        .args(["check", "-", "--database-url", secret_url])
        .env_remove("PGPREFLIGHT_DATABASE_URL")
        .env_remove("DATABASE_URL")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("child stdin")
                .write_all(b"SELECT 'literal-marker';")?;
            child.wait_with_output()
        })
        .expect("run pgpreflight");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("database connection failed"));
    assert!(!stderr.contains("literal-marker"));
    assert!(!stderr.contains("credential-marker"));
    assert!(!stderr.contains(secret_url));
    assert!(!stderr.contains("Connection refused"));
}
