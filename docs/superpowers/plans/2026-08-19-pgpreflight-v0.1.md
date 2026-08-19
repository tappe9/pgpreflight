# pgpreflight v0.1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and publish `pgpreflight` v0.1 as a safe Rust CLI/library that checks one literal PostgreSQL `SELECT`, `UPDATE`, or `DELETE` statement against the connected server's real planner without intentionally executing the target DML.

**Architecture:** Use a three-crate Cargo workspace. `pgpreflight-core` owns stable normalized models, configuration, diagnostics, rules, and JSON report types; `pgpreflight-postgres` owns `sqlparser-rs` validation, PostgreSQL access, Safe Mode, catalog reads, and plan normalization; `pgpreflight` is a thin CLI. Implement each slice with TDD and keep PostgreSQL-driver, parser-AST, and raw-plan types out of the public core API.

**Tech Stack:** Rust, Cargo workspace, `sqlparser-rs`, `tokio`, `tokio-postgres`, `clap`, `serde`, `serde_json`, `toml`, `thiserror`, `assert_cmd`, `predicates`, `jsonschema`, GitHub Actions, PostgreSQL 14–18.

**Spec:** `docs/superpowers/specs/2026-08-19-pgpreflight-design.md`

## Global Constraints

- v0.1 accepts exactly one complete literal `SELECT`, `UPDATE`, or `DELETE` statement.
- PostgreSQL connection is required; PostgreSQL remains authoritative for semantic validation and planning.
- Use plain `EXPLAIN (FORMAT JSON, VERBOSE TRUE)` only; never use `EXPLAIN ANALYZE`.
- Every planning operation runs in a read-only transaction with `SET LOCAL statement_timeout = '3000ms'` and `SET LOCAL lock_timeout = '500ms'` by default.
- Supported PostgreSQL majors: 14, 15, 16, 17, 18.
- Supported OS targets: Linux x86_64, macOS aarch64/x86_64, Windows x86_64.
- SQL text, SQL literal values, passwords, full credential-bearing URLs, and raw verbose plan expressions must not appear in normal output, public errors, logs, or persisted artifacts.
- `pgpreflight-core` must not expose `sqlparser-rs` AST types, PostgreSQL driver types, raw plan JSON, or an async runtime in its public API.
- JSON output uses `schema_version = 1`; existing v1 field names, types, and meanings are stable.
- Default exit policy: clean/warning-only = 0, error diagnostic = 1, tool/config/connection/parse failure = 2; `--fail-on warning` changes warning-only to 1.
- Rules are `PGP001`, `PGP002`, `PGP101`, `PGP102`, `PGP103`, and `PGP104` with the thresholds approved in the spec.
- Ambiguous evidence must cause a rule to skip rather than guess.
- License: `MIT OR Apache-2.0`.
- No telemetry, crash upload, SQL history, query execution, SQL rewriting, automatic index creation, DDL checking, parameter binding, batch mode, VS Code extension, or Web UI in v0.1.

---

## File and Responsibility Map

```text
Cargo.toml                              Workspace members, shared package metadata, workspace dependencies
rust-toolchain.toml                    Pinned stable toolchain for development/CI
LICENSE-MIT                            MIT license text
LICENSE-APACHE                         Apache-2.0 license text

crates/pgpreflight-core/
  Cargo.toml                           Core crate metadata
  src/lib.rs                           Public exports only
  src/model.rs                         Stable normalized statement/plan/relation models
  src/config.rs                        Strict versioned configuration and defaults
  src/diagnostic.rs                    Rule IDs, severity, typed evidence, report model
  src/rules/mod.rs                     Rule dispatcher and deterministic ordering
  src/rules/missing_where.rs           PGP001/PGP002
  src/rules/large_affected.rs          PGP101
  src/rules/large_seq_scan.rs          PGP102
  src/rules/large_result.rs            PGP103
  src/rules/cartesian_join.rs          PGP104

crates/pgpreflight-postgres/
  Cargo.toml                           PostgreSQL adapter dependencies
  src/lib.rs                           Facade exports
  src/parser.rs                        sqlparser-rs PostgreSQL dialect parsing
  src/validation.rs                    One-statement allowlist and unsafe-construct rejection
  src/error.rs                         Sanitized adapter error taxonomy
  src/client.rs                        Connection and PgPreflight facade
  src/safe_mode.rs                     Read-only transaction/timeouts/rollback orchestration
  src/explain.rs                       Plain EXPLAIN JSON request and safe raw decoding
  src/catalog.rs                       Relation statistics lookup
  src/normalize.rs                     Raw plan -> pgpreflight-core normalized model
  src/join_graph.rs                    Conservative AST join-graph extraction for PGP104

crates/pgpreflight/
  Cargo.toml                           CLI package/binary metadata
  src/main.rs                          Async entry point and final exit code only
  src/args.rs                          clap command model
  src/input.rs                         UTF-8/BOM/stdin/file input handling
  src/config_file.rs                   pgpreflight.toml discovery and loading
  src/database_url.rs                  URL precedence and sanitized display
  src/render/mod.rs                    Renderer selection
  src/render/text.rs                   Human-readable report
  src/render/json.rs                   Schema-v1 JSON output and failure envelope
  src/exit.rs                          fail-on policy -> process exit code

schemas/report-v1.schema.json          Machine-readable JSON output contract

tests/sql-corpus/accepted/             Supported SQL fixtures
tests/sql-corpus/rejected/             Explicitly rejected SQL fixtures
tests/sql-corpus/known-unsupported/     PostgreSQL-valid syntax not safely supported yet

tests/postgres/schema.sql              Integration-test schema
tests/postgres/seed.sql                Deterministic data used before ANALYZE

docs/REQUIREMENTS.md                   Testable v0.1 requirements
docs/API-DESIGN.md                     Public Rust API contract
docs/RULES.md                          Rule semantics and thresholds
docs/SAFETY.md                         Safety boundary and least-privilege guidance
docs/JSON-SCHEMA.md                    Schema-v1 compatibility policy
docs/COMPATIBILITY.md                  PostgreSQL/OS/parser compatibility
docs/ROADMAP.md                        Explicit post-v0.1 work

.github/workflows/ci.yml                Format, clippy, unit, PostgreSQL matrix, cross-platform checks
.github/workflows/release.yml           Tagged release build/package/checksum flow
.github/dependabot.yml                  Cargo/GitHub Actions dependency updates
CONTRIBUTING.md                         Contribution workflow and sensitive-data guidance
SECURITY.md                             Private vulnerability reporting scope
CHANGELOG.md                            Release history
```

---

### Task 1: Establish the Rust workspace and OSS foundation

**Issue title:** `Bootstrap Cargo workspace and OSS project foundation`

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `LICENSE-MIT`
- Create: `LICENSE-APACHE`
- Create: `crates/pgpreflight-core/Cargo.toml`
- Create: `crates/pgpreflight-core/src/lib.rs`
- Create: `crates/pgpreflight-postgres/Cargo.toml`
- Create: `crates/pgpreflight-postgres/src/lib.rs`
- Create: `crates/pgpreflight/Cargo.toml`
- Create: `crates/pgpreflight/src/main.rs`
- Create: `.github/workflows/ci.yml`
- Modify: `README.md`

**Interfaces:**
- Produces: workspace crates named `pgpreflight-core`, `pgpreflight-postgres`, and `pgpreflight`; binary `pgpreflight`.
- Produces: dependency direction `pgpreflight -> pgpreflight-postgres -> pgpreflight-core`.
- Consumes: approved spec only.

- [ ] **Step 1: Add a workspace smoke test by making each crate compile with a minimal public marker**

```rust
// crates/pgpreflight-core/src/lib.rs
#![forbid(unsafe_code)]

pub const CRATE_NAME: &str = "pgpreflight-core";
```

```rust
// crates/pgpreflight-postgres/src/lib.rs
#![forbid(unsafe_code)]

pub const CRATE_NAME: &str = "pgpreflight-postgres";
```

```rust
// crates/pgpreflight/src/main.rs
fn main() {
    println!("pgpreflight");
}
```

- [ ] **Step 2: Create the workspace manifest and crate manifests**

```toml
# Cargo.toml
[workspace]
members = [
  "crates/pgpreflight-core",
  "crates/pgpreflight-postgres",
  "crates/pgpreflight",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/tappe9/pgpreflight"
rust-version = "1.85"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
```

Each crate manifest must inherit `version`, `edition`, `license`, `repository`, and `rust-version` from the workspace. Do not add parser/database/CLI dependencies until their owning task.

- [ ] **Step 3: Run the workspace build and tests**

Run:

```bash
cargo check --workspace
cargo test --workspace
```

Expected: both commands exit 0.

- [ ] **Step 4: Add MIT and Apache-2.0 license files and update README status**

README must state that implementation has begun, list the three crates, and retain the warning that plain `EXPLAIN` is not a universal sandbox.

- [ ] **Step 5: Add baseline CI**

`.github/workflows/ci.yml` must initially run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

on Ubuntu, and `cargo check --workspace --all-targets` on macOS and Windows.

- [ ] **Step 6: Verify the foundation**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Expected: all exit 0.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml rust-toolchain.toml LICENSE-MIT LICENSE-APACHE crates .github/workflows/ci.yml README.md
git commit -m "chore: bootstrap pgpreflight workspace"
```

---

### Task 2: Implement core models, strict configuration, and JSON report contract

**Issue title:** `Define core models, configuration, and JSON schema v1`

**Files:**
- Create: `crates/pgpreflight-core/src/model.rs`
- Create: `crates/pgpreflight-core/src/config.rs`
- Create: `crates/pgpreflight-core/src/diagnostic.rs`
- Modify: `crates/pgpreflight-core/src/lib.rs`
- Create: `crates/pgpreflight-core/tests/config.rs`
- Create: `crates/pgpreflight-core/tests/report_json.rs`
- Create: `schemas/report-v1.schema.json`

**Interfaces:**
- Produces: `StatementKind`, `RelationRef`, `StatementFacts`, `NormalizedPlan`, `PlanNode`, `PlanNodeKind`, `RelationStats`, `AnalysisInput`.
- Produces: `Config::default()`, strict deserialization of config version 1.
- Produces: `RuleId`, `Severity`, `DiagnosticEvidence`, `Diagnostic`, `Report`, `ReportStatus`.
- Later tasks must use these types instead of raw parser/driver JSON.

- [ ] **Step 1: Write failing tests for configuration defaults and strict deserialization**

```rust
#[test]
fn default_thresholds_match_v01_contract() {
    let config = pgpreflight_core::Config::default();
    assert_eq!(config.postgres.statement_timeout_ms, 3_000);
    assert_eq!(config.postgres.lock_timeout_ms, 500);
    assert_eq!(config.rules.pgp101.max_rows, 10_000.0);
    assert_eq!(config.rules.pgp101.max_table_ratio, 0.05);
    assert_eq!(config.rules.pgp102.min_relation_rows, 100_000.0);
    assert_eq!(config.rules.pgp103.max_result_rows, 100_000.0);
}

#[test]
fn unknown_config_key_is_rejected() {
    let text = "version = 1\nunknown = true\n";
    let result = toml::from_str::<pgpreflight_core::Config>(text);
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p pgpreflight-core --test config
```

Expected: compile failure because `Config` does not exist.

- [ ] **Step 3: Implement the normalized model and strict config types**

Use `#[serde(deny_unknown_fields)]` on versioned configuration structs. Validate ratios in `Config::validate()`:

```rust
pub fn validate(&self) -> Result<(), ConfigError> {
    if self.version != 1 {
        return Err(ConfigError::UnsupportedVersion(self.version));
    }
    if !(0.0..=1.0).contains(&self.rules.pgp101.max_table_ratio) {
        return Err(ConfigError::InvalidRatio("rules.PGP101.max_table_ratio"));
    }
    if !(0.0..=1.0).contains(&self.rules.pgp102.max_output_ratio) {
        return Err(ConfigError::InvalidRatio("rules.PGP102.max_output_ratio"));
    }
    Ok(())
}
```

- [ ] **Step 4: Write failing JSON contract test**

```rust
#[test]
fn clean_report_serializes_as_schema_v1() {
    let report = Report::clean(StatementKind::Select);
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["status"], "clean");
    assert_eq!(value["summary"]["errors"], 0);
    assert_eq!(value["summary"]["warnings"], 0);
    assert_eq!(value["diagnostics"], serde_json::json!([]));
}
```

- [ ] **Step 5: Implement typed diagnostics and report model**

`DiagnosticEvidence` must use internally tagged serde representation:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiagnosticEvidence {
    MissingWhere { /* relation and optional estimate */ },
    LargeAffectedRows { /* approved evidence fields */ },
    LargeSequentialScan { /* approved evidence fields */ },
    LargeResultSet { estimated_result_rows: f64 },
    CartesianJoin { /* disconnected groups and optional result rows */ },
}
```

- [ ] **Step 6: Add and validate `schemas/report-v1.schema.json`**

The schema must require `schema_version`, `tool`, `status`, `statement`, `summary`, `diagnostics`, and `failure`; allow `statement` and `failure` to be null where specified; and allow future optional fields with `additionalProperties: true` at extension-safe levels.

- [ ] **Step 7: Run core tests**

Run:

```bash
cargo test -p pgpreflight-core
cargo clippy -p pgpreflight-core --all-targets -- -D warnings
```

Expected: both exit 0.

- [ ] **Step 8: Commit**

```bash
git add crates/pgpreflight-core schemas/report-v1.schema.json
git commit -m "feat: define core analysis contracts"
```

---

### Task 3: Parse and conservatively validate supported SQL

**Issue title:** `Parse and safely validate SELECT UPDATE and DELETE`

**Files:**
- Modify: `crates/pgpreflight-postgres/Cargo.toml`
- Create: `crates/pgpreflight-postgres/src/parser.rs`
- Create: `crates/pgpreflight-postgres/src/validation.rs`
- Create: `crates/pgpreflight-postgres/src/error.rs`
- Modify: `crates/pgpreflight-postgres/src/lib.rs`
- Create: `crates/pgpreflight-postgres/tests/sql_validation.rs`
- Create fixtures under: `tests/sql-corpus/accepted/`, `tests/sql-corpus/rejected/`, `tests/sql-corpus/known-unsupported/`

**Interfaces:**
- Produces: `parse_and_validate(sql: &str) -> Result<ValidatedStatement, CheckError>`.
- Produces: `ValidatedStatement::facts() -> &StatementFacts` and private access to parser AST for adapter-only work.
- Consumes: `pgpreflight_core::StatementFacts`, `StatementKind`, `RelationRef`.

- [ ] **Step 1: Write failing acceptance/rejection tests**

```rust
#[test]
fn accepts_single_select() {
    let validated = parse_and_validate("SELECT * FROM orders WHERE id = 1").unwrap();
    assert_eq!(validated.facts().kind, StatementKind::Select);
}

#[test]
fn rejects_multiple_statements() {
    let error = parse_and_validate("SELECT 1; SELECT 2;").unwrap_err();
    assert!(matches!(error, CheckError::MultipleStatements));
}

#[test]
fn rejects_locking_select() {
    let error = parse_and_validate("SELECT * FROM orders FOR UPDATE").unwrap_err();
    assert!(matches!(error, CheckError::UnsafeConstruct { .. }));
}
```

Also add tests for `INSERT`, `MERGE`, `COPY`, `CALL`, `DO`, DDL, transaction control, and direct `EXPLAIN` input.

- [ ] **Step 2: Run the focused validation test and verify failure**

Run:

```bash
cargo test -p pgpreflight-postgres --test sql_validation
```

Expected: compile failure because parser/validator APIs do not exist.

- [ ] **Step 3: Add `sqlparser-rs` and implement PostgreSQL-dialect parsing**

Require exactly one parsed statement. Treat parser failure as sanitized `CheckError::SqlParse`; never embed the original SQL in `Display`.

- [ ] **Step 4: Implement recursive safety validation**

Validation must inspect CTEs and nested statement-bearing constructs so unsupported modifying statements cannot bypass the outer-statement allowlist. Any AST construct whose safety cannot be established must return `CheckError::UnsupportedStatement` or `UnsafeConstruct` rather than being passed to PostgreSQL.

- [ ] **Step 5: Extract only safe statement facts**

Populate:

```rust
StatementFacts {
    kind,
    target_relation,
    has_where,
    has_returning,
    join_graph: JoinGraph::default(),
}
```

Do not store SQL strings or literal-expression text.

- [ ] **Step 6: Add corpus-driven regression tests**

Each fixture in `accepted` must parse successfully, each fixture in `rejected` must return the expected stable category, and each `known-unsupported` fixture must remain explicitly documented rather than silently accepted.

- [ ] **Step 7: Verify parser crate tests**

Run:

```bash
cargo test -p pgpreflight-postgres --test sql_validation
cargo clippy -p pgpreflight-postgres --all-targets -- -D warnings
```

Expected: both exit 0.

- [ ] **Step 8: Commit**

```bash
git add crates/pgpreflight-postgres tests/sql-corpus
git commit -m "feat: validate supported PostgreSQL statements"
```

---

### Task 4: Implement PostgreSQL connection, Safe Mode, and sanitized failures

**Issue title:** `Add PostgreSQL Safe Mode planning adapter`

**Files:**
- Modify: `crates/pgpreflight-postgres/Cargo.toml`
- Create: `crates/pgpreflight-postgres/src/client.rs`
- Create: `crates/pgpreflight-postgres/src/safe_mode.rs`
- Create: `crates/pgpreflight-postgres/src/explain.rs`
- Modify: `crates/pgpreflight-postgres/src/error.rs`
- Modify: `crates/pgpreflight-postgres/src/lib.rs`
- Create: `crates/pgpreflight-postgres/tests/safe_mode.rs`
- Create: `tests/postgres/schema.sql`
- Create: `tests/postgres/seed.sql`

**Interfaces:**
- Produces: `PgPreflight::connect(database_url: &str) -> Result<PgPreflight, CheckError>`.
- Produces internal: `SafeSession::explain(&ValidatedStatement, &Config) -> Result<RawPlan, CheckError>`.
- `RawPlan` stays private to `pgpreflight-postgres`.

- [ ] **Step 1: Write integration test proving target UPDATE is not executed**

```rust
#[tokio::test]
async fn update_is_planned_but_not_executed() {
    let checker = test_checker().await;
    let before = fetch_status(1).await;

    checker
        .explain_only("UPDATE orders SET status = 'done' WHERE id = 1", &Config::default())
        .await
        .unwrap();

    let after = fetch_status(1).await;
    assert_eq!(before, after);
}
```

- [ ] **Step 2: Run the test against PostgreSQL and verify failure**

Run with a local test URL:

```bash
PGPREFLIGHT_TEST_DATABASE_URL=postgresql://postgres:postgres@localhost/pgpreflight_test \
  cargo test -p pgpreflight-postgres --test safe_mode update_is_planned_but_not_executed -- --nocapture
```

Expected: compile failure because `PgPreflight`/`explain_only` does not exist.

- [ ] **Step 3: Add `tokio` and `tokio-postgres`; implement connection and read-only transaction**

The transaction sequence must be functionally equivalent to:

```sql
BEGIN READ ONLY;
SET LOCAL statement_timeout = '3000ms';
SET LOCAL lock_timeout = '500ms';
EXPLAIN (FORMAT JSON, VERBOSE TRUE) <validated statement>;
ROLLBACK;
```

Never concatenate anything except the already validated complete statement after the fixed `EXPLAIN` prefix. Never use `ANALYZE`.

- [ ] **Step 4: Add tests for read-only and timeout behavior**

Use `SHOW transaction_read_only` inside the test-only session hook and assert `on`. Add a lock-contention fixture and assert timeout maps to `CheckError::Timeout` without including raw driver text.

- [ ] **Step 5: Add rollback-on-error regression test**

Force a planner error after beginning the session and then verify the connection can start a fresh transaction; no transaction may remain aborted/open.

- [ ] **Step 6: Add redaction tests**

Use literals such as `private@example.com`, `secret-token`, and a URL containing `super-secret`. Assert none appears in `Display`, `Debug` for public wrapper errors, or rendered adapter failure messages.

- [ ] **Step 7: Verify Safe Mode tests**

Run:

```bash
cargo test -p pgpreflight-postgres --test safe_mode
cargo clippy -p pgpreflight-postgres --all-targets -- -D warnings
```

Expected: exit 0 with a configured test database.

- [ ] **Step 8: Commit**

```bash
git add crates/pgpreflight-postgres tests/postgres
git commit -m "feat: add safe PostgreSQL planning session"
```

---

### Task 5: Normalize EXPLAIN JSON and catalog relation statistics

**Issue title:** `Normalize PostgreSQL plans and relation statistics`

**Files:**
- Create: `crates/pgpreflight-postgres/src/catalog.rs`
- Create: `crates/pgpreflight-postgres/src/normalize.rs`
- Modify: `crates/pgpreflight-postgres/src/explain.rs`
- Modify: `crates/pgpreflight-postgres/src/client.rs`
- Create: `crates/pgpreflight-postgres/tests/normalize.rs`

**Interfaces:**
- Produces: `normalize_plan(raw: &RawPlan) -> Result<NormalizedPlan, CheckError>`.
- Produces: `load_relation_stats(...) -> Result<Vec<RelationStats>, CheckError>`.
- Produces: `PgPreflight::analysis_input(sql, config) -> Result<AnalysisInput, CheckError>` as the adapter boundary used by later rule/CLI tasks.

- [ ] **Step 1: Write failing plan-normalization tests with representative JSON fixtures**

```rust
#[test]
fn normalizes_seq_scan_without_retaining_filter_text() {
    let plan = normalize_fixture("seq_scan.json").unwrap();
    assert_eq!(plan.root.kind, PlanNodeKind::SeqScan);
    assert_eq!(plan.root.relation.as_ref().unwrap().name, "orders");
    assert_eq!(plan.root.estimated_rows, 21_840.0);
}
```

The fixture should contain a sensitive `Filter` expression and the normalized type must expose no field capable of returning it.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```bash
cargo test -p pgpreflight-postgres --test normalize
```

Expected: compile failure because normalization API does not exist.

- [ ] **Step 3: Implement tolerant plan-node normalization**

Map known node names to stable variants and unknown node names to `PlanNodeKind::Other(String)`. Missing mandatory structural fields returns `CheckError::UnsupportedPlan`; unknown extra JSON keys are ignored.

- [ ] **Step 4: Implement UPDATE/DELETE affected-row extraction**

For `ModifyTable`, derive the estimate entering the modification from its child shape, including `Append` for partitioned targets. If a shape cannot be interpreted confidently, set `estimated_affected_rows = None`.

- [ ] **Step 5: Implement catalog relation-stat lookup**

Query `pg_class`/`pg_namespace` using relation identity resolved from verbose plan metadata. Return `estimated_live_rows` and pages as optional values; missing or non-positive stats must not be fabricated.

- [ ] **Step 6: Add PostgreSQL integration assertions**

After test seed and `ANALYZE`, assert normalization preserves relation/schema/alias and produces non-sensitive estimates. Avoid exact cost snapshots across server majors.

- [ ] **Step 7: Verify adapter tests**

Run:

```bash
cargo test -p pgpreflight-postgres
cargo clippy -p pgpreflight-postgres --all-targets -- -D warnings
```

Expected: all tests exit 0 with configured integration DB.

- [ ] **Step 8: Commit**

```bash
git add crates/pgpreflight-postgres
git commit -m "feat: normalize PostgreSQL planner evidence"
```

---

### Task 6: Implement PGP001, PGP002, and PGP101 with TDD

**Issue title:** `Detect unsafe and large UPDATE DELETE targets`

**Files:**
- Create: `crates/pgpreflight-core/src/rules/mod.rs`
- Create: `crates/pgpreflight-core/src/rules/missing_where.rs`
- Create: `crates/pgpreflight-core/src/rules/large_affected.rs`
- Modify: `crates/pgpreflight-core/src/lib.rs`
- Create: `crates/pgpreflight-core/tests/update_delete_rules.rs`

**Interfaces:**
- Produces: `analyze(input: &AnalysisInput, config: &Config) -> Report`.
- Produces PGP001/PGP002 errors and PGP101 warnings with typed evidence.

- [ ] **Step 1: Write failing PGP001/PGP002 tests**

```rust
#[test]
fn pgp001_fires_for_update_without_where() {
    let input = fixture_update(false, Some(42_000.0), Some(1_000_000.0));
    let report = analyze(&input, &Config::default());
    assert_eq!(report.diagnostics[0].rule_id, RuleId::Pgp001);
    assert_eq!(report.diagnostics[0].severity, Severity::Error);
}

#[test]
fn pgp001_does_not_fire_when_where_is_syntactically_present() {
    let input = fixture_update(true, Some(42_000.0), Some(1_000_000.0));
    let report = analyze(&input, &Config::default());
    assert!(!report.diagnostics.iter().any(|d| d.rule_id == RuleId::Pgp001));
}
```

Add equivalent DELETE tests and explicit `WHERE TRUE` fact behavior.

- [ ] **Step 2: Run and verify failure**

Run:

```bash
cargo test -p pgpreflight-core --test update_delete_rules
```

Expected: failure because rule dispatcher is not implemented.

- [ ] **Step 3: Implement missing-WHERE rules minimally**

Rules must fire even when affected-row evidence is unavailable.

- [ ] **Step 4: Write failing PGP101 boundary tests**

Cover `9_999`, `10_000`, `10_001`; ratio `0.04999`, `0.05`, `0.05001`; missing relation stats; and `min_rows_for_ratio = 1_000`.

- [ ] **Step 5: Implement PGP101 exactly from config**

Trigger on absolute threshold OR ratio threshold. Evidence must record which trigger(s) fired.

- [ ] **Step 6: Implement deterministic report ordering and summary counts**

Order: error before warning, then rule ID, relation identity, plan traversal order.

- [ ] **Step 7: Verify core rule tests**

Run:

```bash
cargo test -p pgpreflight-core
cargo clippy -p pgpreflight-core --all-targets -- -D warnings
```

Expected: exit 0.

- [ ] **Step 8: Commit**

```bash
git add crates/pgpreflight-core
git commit -m "feat: detect risky update and delete targets"
```

---

### Task 7: Implement PGP102 and PGP103 planner-volume rules

**Issue title:** `Detect large sequential scans and result sets`

**Files:**
- Create: `crates/pgpreflight-core/src/rules/large_seq_scan.rs`
- Create: `crates/pgpreflight-core/src/rules/large_result.rs`
- Modify: `crates/pgpreflight-core/src/rules/mod.rs`
- Create: `crates/pgpreflight-core/tests/planner_volume_rules.rs`

**Interfaces:**
- Adds PGP102 per Seq Scan node and PGP103 for SELECT root output.
- Consumes only normalized models and relation stats.

- [ ] **Step 1: Write failing PGP102 tests**

Cover:

```text
relation 15,284,129 / output 21,840 -> warning
relation 99,999 / output 100 -> no warning
relation 1,000,000 / output 500,000 -> no warning at 20% threshold
IndexScan -> no warning
missing relation stats -> no warning
self join with two SeqScan nodes -> independent diagnostics
```

- [ ] **Step 2: Run and verify failure**

Run:

```bash
cargo test -p pgpreflight-core --test planner_volume_rules pgp102
```

Expected: failing assertions because PGP102 is not implemented.

- [ ] **Step 3: Implement PGP102 conservatively**

Use catalog relation rows as the approximate scanned-row count and scan-node `estimated_rows` as output. Skip zero/negative/missing relation estimates.

- [ ] **Step 4: Write failing PGP103 tests**

Cover `99_999`, `100_000`, `100_001`, root Limit output of 100, Aggregate output below threshold, and UPDATE/DELETE with RETURNING excluded.

- [ ] **Step 5: Implement PGP103 using root estimated rows for SELECT only**

Do not infer network byte size or execution time.

- [ ] **Step 6: Verify planner-volume rules**

Run:

```bash
cargo test -p pgpreflight-core --test planner_volume_rules
cargo test -p pgpreflight-core
```

Expected: exit 0.

- [ ] **Step 7: Commit**

```bash
git add crates/pgpreflight-core
git commit -m "feat: detect large planner row volumes"
```

---

### Task 8: Build conservative join graph and implement PGP104

**Issue title:** `Detect provable Cartesian joins conservatively`

**Files:**
- Create: `crates/pgpreflight-postgres/src/join_graph.rs`
- Modify: `crates/pgpreflight-postgres/src/parser.rs`
- Create: `crates/pgpreflight-postgres/tests/join_graph.rs`
- Create: `crates/pgpreflight-core/src/rules/cartesian_join.rs`
- Modify: `crates/pgpreflight-core/src/rules/mod.rs`
- Create: `crates/pgpreflight-core/tests/cartesian_rule.rs`

**Interfaces:**
- PostgreSQL adapter populates `StatementFacts.join_graph` without storing expression text.
- Core PGP104 consumes only that graph and optional root estimated rows.

- [ ] **Step 1: Write failing join-graph extraction tests**

Required cases:

```sql
SELECT * FROM customers c, orders o;
-- two disconnected components

SELECT * FROM customers c, orders o WHERE c.id = o.customer_id;
-- one connected component

SELECT * FROM customers c JOIN orders o USING (customer_id);
-- connected

SELECT * FROM customers c CROSS JOIN orders o;
-- disconnected unless a later supported predicate connects them
```

Also cover aliases in self joins.

- [ ] **Step 2: Run and verify failure**

Run:

```bash
cargo test -p pgpreflight-postgres --test join_graph
```

Expected: failure because join graph extraction is absent.

- [ ] **Step 3: Implement relation-occurrence graph extraction**

Qualified cross-relation predicates add edges. `USING` and `NATURAL JOIN` connect operands. `CROSS JOIN` and `ON TRUE` do not add an edge by themselves. If unqualified columns, LATERAL, correlated-subquery ownership, or unsupported set-returning constructs prevent confident ownership resolution, mark that query level indeterminate.

- [ ] **Step 4: Write failing core PGP104 tests**

```rust
#[test]
fn pgp104_fires_for_two_disconnected_groups() {
    let input = analysis_with_join_graph(graph_with_components(2));
    let report = analyze(&input, &Config::default());
    assert!(report.diagnostics.iter().any(|d| d.rule_id == RuleId::Pgp104));
}

#[test]
fn pgp104_skips_indeterminate_graph() {
    let input = analysis_with_join_graph(JoinGraph::Indeterminate);
    let report = analyze(&input, &Config::default());
    assert!(!report.diagnostics.iter().any(|d| d.rule_id == RuleId::Pgp104));
}
```

- [ ] **Step 5: Implement PGP104 typed evidence**

Evidence returns disconnected groups by safe relation identity/alias and optional estimated result rows; it must not include predicate text.

- [ ] **Step 6: Verify adapter and core tests**

Run:

```bash
cargo test -p pgpreflight-postgres --test join_graph
cargo test -p pgpreflight-core --test cartesian_rule
cargo test --workspace
```

Expected: exit 0 with DB-independent tests.

- [ ] **Step 7: Commit**

```bash
git add crates/pgpreflight-postgres crates/pgpreflight-core
git commit -m "feat: detect possible Cartesian joins"
```

---

### Task 9: Implement CLI input, config discovery, text/JSON output, and exit policy

**Issue title:** `Implement pgpreflight check CLI contract`

**Files:**
- Modify: `crates/pgpreflight/Cargo.toml`
- Replace: `crates/pgpreflight/src/main.rs`
- Create: `crates/pgpreflight/src/args.rs`
- Create: `crates/pgpreflight/src/input.rs`
- Create: `crates/pgpreflight/src/config_file.rs`
- Create: `crates/pgpreflight/src/database_url.rs`
- Create: `crates/pgpreflight/src/render/mod.rs`
- Create: `crates/pgpreflight/src/render/text.rs`
- Create: `crates/pgpreflight/src/render/json.rs`
- Create: `crates/pgpreflight/src/exit.rs`
- Create: `crates/pgpreflight/tests/cli.rs`

**Interfaces:**
- Produces CLI: `pgpreflight check <INPUT> [--database-url <URL>] [--config <PATH>] [--format text|json] [--fail-on error|warning]`.
- Database URL precedence: CLI > `PGPREFLIGHT_DATABASE_URL` > `DATABASE_URL`.
- Config discovery: explicit path > cwd > parent directories > built-ins; no merging.

- [ ] **Step 1: Write failing CLI argument and input tests**

Use `assert_cmd` to cover file input, `-` stdin, empty input, comment-only input, BOM stripping, invalid UTF-8, and multiple statements.

- [ ] **Step 2: Run and verify failure**

Run:

```bash
cargo test -p pgpreflight --test cli
```

Expected: failures because the CLI contract is absent.

- [ ] **Step 3: Implement clap command model and input loading**

Default `--format text`, default `--fail-on error`. Read stdin to EOF. Strip UTF-8 BOM once. Do not echo the input on errors.

- [ ] **Step 4: Write failing config-discovery tests**

Create temporary nested directories and assert closest ancestor `pgpreflight.toml` is selected, `--config` wins, files are not merged, unknown keys/version are exit-2 tool failures.

- [ ] **Step 5: Implement config and database URL resolution**

Never print the full URL. A sanitized representation may include host/port/database and username but must replace password with `****`.

- [ ] **Step 6: Write failing text/JSON/exit-code contract tests**

Required cases:

```text
clean -> exit 0
warning only -> exit 0
warning + --fail-on warning -> exit 1
error diagnostic -> exit 1
connection/config/parse failure -> exit 2
```

For JSON, deserialize stdout and assert `schema_version = 1`; stderr must be empty for normal structured failures.

- [ ] **Step 7: Implement text renderer, JSON renderer, and exit policy**

Text output must not contain SQL or literals. JSON output must be exactly one top-level object on stdout. TTY-only color and `NO_COLOR` handling belong only to text rendering.

- [ ] **Step 8: Verify CLI contract**

Run:

```bash
cargo test -p pgpreflight
cargo clippy -p pgpreflight --all-targets -- -D warnings
```

Expected: exit 0.

- [ ] **Step 9: Commit**

```bash
git add crates/pgpreflight
git commit -m "feat: implement pgpreflight check CLI"
```

---

### Task 10: Add PostgreSQL 14–18 matrix, security contracts, and cross-platform CI

**Issue title:** `Verify PostgreSQL 14-18 and cross-platform safety contracts`

**Files:**
- Modify: `.github/workflows/ci.yml`
- Create: `crates/pgpreflight-postgres/tests/postgres_compat.rs`
- Create: `crates/pgpreflight/tests/security_contract.rs`
- Create: `crates/pgpreflight/tests/schema_contract.rs`
- Modify: `tests/postgres/schema.sql`
- Modify: `tests/postgres/seed.sql`

**Interfaces:**
- Produces required CI jobs: `format`, `clippy`, `unit-tests`, `postgres-14`, `postgres-15`, `postgres-16`, `postgres-17`, `postgres-18`, `cross-platform`, `security-contract`, `schema-contract`.

- [ ] **Step 1: Add a PostgreSQL-major integration test suite**

Test real `SELECT`, `UPDATE`, `DELETE`, quoted identifiers, schema-qualified relations, CTE, `UPDATE ... FROM`, `DELETE ... USING`, and partitioned-table plan shapes. Assert semantic normalized fields, not exact cost numbers.

- [ ] **Step 2: Add explicit secret-leak contract tests**

Inject:

```text
private@example.com
secret-token
super-secret
```

into SQL literals/connection URLs and force parser, planner, catalog, timeout, and connection failures. Search stdout/stderr/public error formatting and assert all three strings are absent.

- [ ] **Step 3: Add schema-v1 validation test**

Validate clean, warning, error-diagnostic, and tool-failure JSON examples against `schemas/report-v1.schema.json` using `jsonschema` in tests.

- [ ] **Step 4: Expand GitHub Actions**

Linux runs PostgreSQL 14–18 as separate matrix entries. Ubuntu/macOS/Windows run build/unit/parser/CLI non-DB tests. Add MSRV `cargo check --workspace` using the chosen `rust-version`.

- [ ] **Step 5: Run the full local quality gate available on the development machine**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

If Docker is available, additionally run integration tests against PostgreSQL 14, 15, 16, 17, and 18 sequentially. Expected: all configured checks exit 0.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/ci.yml crates tests schemas
git commit -m "test: verify compatibility and safety contracts"
```

---

### Task 11: Complete OSS documentation and release pipeline

**Issue title:** `Prepare pgpreflight v0.1 OSS release`

**Files:**
- Create: `docs/REQUIREMENTS.md`
- Create: `docs/API-DESIGN.md`
- Create: `docs/RULES.md`
- Create: `docs/SAFETY.md`
- Create: `docs/JSON-SCHEMA.md`
- Create: `docs/COMPATIBILITY.md`
- Create: `docs/ROADMAP.md`
- Create: `CONTRIBUTING.md`
- Create: `SECURITY.md`
- Create: `CHANGELOG.md`
- Create: `.github/dependabot.yml`
- Create: `.github/workflows/release.yml`
- Modify: `README.md`
- Modify: all three crate `Cargo.toml` files

**Interfaces:**
- Produces documented public contracts for users/contributors and release artifacts for four target families.
- Produces crates.io metadata sufficient for `cargo publish --dry-run` on all three crates.

- [ ] **Step 1: Write documentation from the approved spec and implemented behavior**

Each of the six rules must document severity, target statements, trigger formula, default threshold, positive example, negative example, and known limitation. `SAFETY.md` must explicitly distinguish plain `EXPLAIN` from `EXPLAIN ANALYZE` and state that planner hooks, FDWs, and incorrectly declared immutable functions are outside a universal sandbox guarantee.

- [ ] **Step 2: Add crates.io metadata and package exclusions**

Each published crate must include description, repository, documentation/readme, keywords/categories where appropriate, and `license = "MIT OR Apache-2.0"`. Verify package contents do not include secrets, local fixtures with credentials, or unnecessary CI artifacts.

- [ ] **Step 3: Add release workflow**

On `v*` tag, build release binaries for:

```text
x86_64-unknown-linux-gnu
x86_64-pc-windows-msvc
x86_64-apple-darwin
aarch64-apple-darwin
```

Package binaries with README/license files and generate SHA-256 checksums. Crates.io publication remains an explicit release step; do not store crates.io tokens in code or logs.

- [ ] **Step 4: Run package dry-runs in dependency order**

Run:

```bash
cargo publish -p pgpreflight-core --dry-run
cargo publish -p pgpreflight-postgres --dry-run
cargo publish -p pgpreflight --dry-run
```

Expected: all three succeed before v0.1.0 is tagged.

- [ ] **Step 5: Run final quality gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check --workspace --all-targets --all-features
```

Expected: all exit 0. Confirm GitHub Actions PostgreSQL 14–18, cross-platform, schema-contract, and security-contract checks are green before release.

- [ ] **Step 6: Commit**

```bash
git add README.md docs CONTRIBUTING.md SECURITY.md CHANGELOG.md .github crates/*/Cargo.toml
git commit -m "docs: prepare pgpreflight v0.1 release"
```

---

## Implementation Order and Review Gates

Execute Tasks 1–11 strictly in order. Each task is one review gate and should normally map to one GitHub issue and one focused PR. Do not begin the next task until the current task's tests and required checks pass and its diff has been reviewed.

Recommended issue dependency chain:

```text
#1 Workspace foundation
 -> #2 Core contracts
 -> #3 SQL validation
 -> #4 Safe Mode
 -> #5 Plan normalization
 -> #6 UPDATE/DELETE rules
 -> #7 Planner-volume rules
 -> #8 Cartesian-join rule
 -> #9 CLI contract
 -> #10 Compatibility/security CI
 -> #11 Release readiness
```

Tasks 6 and 7 both consume Task 5 and could technically proceed in parallel, but sequential execution is preferred for the first implementation because it keeps the public diagnostic/report contract under one active change at a time.

## Final Acceptance Checklist

Before calling v0.1 implementation complete, verify all of the following with fresh command output and GitHub Actions status:

- [ ] One literal `SELECT`, `UPDATE`, or `DELETE` can be checked against a connected PostgreSQL database.
- [ ] `EXPLAIN ANALYZE` never appears in generated SQL or accepted input.
- [ ] Target UPDATE/DELETE statements are not intentionally executed by the tool.
- [ ] Read-only transaction and local timeout tests pass.
- [ ] PGP001, PGP002, PGP101, PGP102, PGP103, and PGP104 meet boundary tests.
- [ ] Ambiguous plan/join evidence produces skips rather than guessed diagnostics.
- [ ] Text output, JSON schema v1, and exit-code contracts pass.
- [ ] Secret-leak tests pass for SQL literals, passwords, URLs, raw driver errors, and raw verbose plan content.
- [ ] PostgreSQL 14, 15, 16, 17, and 18 integration jobs pass.
- [ ] Linux, macOS, and Windows checks pass.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --workspace --all-features` passes.
- [ ] `cargo publish --dry-run` passes for `pgpreflight-core`, `pgpreflight-postgres`, and `pgpreflight`.
- [ ] README, SAFETY, RULES, API-DESIGN, JSON-SCHEMA, COMPATIBILITY, CONTRIBUTING, SECURITY, CHANGELOG, and ROADMAP match implemented behavior.
