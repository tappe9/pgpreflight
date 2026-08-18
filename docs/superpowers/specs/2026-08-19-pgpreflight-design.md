# pgpreflight v0.1 Design Specification

- **Status:** Approved for implementation planning
- **Date:** 2026-08-19
- **Repository:** `tappe9/pgpreflight`
- **License:** `MIT OR Apache-2.0`

## 1. Product definition

`pgpreflight` is an open-source Rust CLI and library that checks PostgreSQL queries against the connected database's real planner before application execution.

The v0.1 position is deliberately narrower than a general SQL linter or migration checker:

> Preflight one literal `SELECT`, `UPDATE`, or `DELETE` statement with plain PostgreSQL `EXPLAIN`, without intentionally executing the target DML.

The tool combines conservative AST validation, `EXPLAIN (FORMAT JSON, VERBOSE TRUE)`, catalog statistics, deterministic rules, human-readable output, and versioned JSON.

## 2. Target user and workflow

The primary user is an application developer working with a local, development, or staging PostgreSQL database.

```bash
export PGPREFLIGHT_DATABASE_URL='postgresql://pgpreflight@localhost/app'
pgpreflight check query.sql
pgpreflight check query.sql --format json
pgpreflight check query.sql --fail-on warning
```

The input contains one complete statement with literal values. Parameter placeholders such as `$1` are not supported in v0.1.

## 3. Goals

v0.1 must:

1. accept exactly one `SELECT`, `UPDATE`, or `DELETE` statement;
2. require a PostgreSQL connection;
3. use the connected server as the semantic and planning authority;
4. use plain `EXPLAIN`, never `EXPLAIN ANALYZE`;
5. use a read-only transaction with local planning and lock timeouts;
6. provide six deterministic diagnostics;
7. provide strict TOML configuration with built-in defaults;
8. provide text and versioned JSON output;
9. avoid leaking SQL literals, passwords, or complete connection URLs;
10. expose reusable Rust crates;
11. test PostgreSQL 14 through 18; and
12. build on Linux, macOS, and Windows.

## 4. Non-goals

v0.1 does not provide:

- SQL execution or `EXPLAIN ANALYZE`;
- measured runtime or exact runtime prediction;
- SQL rewriting, automated fixes, or index creation;
- hypothetical indexes; that belongs to the future `pgindexlab` project;
- DDL or migration analysis;
- `INSERT`, `MERGE`, `COPY`, `CALL`, or `DO` analysis;
- parameter binding;
- batch, glob, directory, or multi-statement processing;
- SARIF, a VS Code extension, or a Web UI;
- telemetry, crash uploads, or query-history storage; or
- a universal sandbox for arbitrary functions, extensions, or FDWs.

## 5. Architecture

```text
pgpreflight/
├── crates/
│   ├── pgpreflight-core/
│   ├── pgpreflight-postgres/
│   └── pgpreflight/
├── docs/
├── schemas/
└── tests/
```

### 5.1 `pgpreflight-core`

Owns normalized models, configuration, rule evaluation, diagnostics, evidence, report status, and serializable JSON report types.

Constraints:

- no PostgreSQL client, CLI, async runtime, `sqlparser-rs` AST, or raw plan JSON in its public API;
- deterministic and database-independent rule tests.

Representative API:

```rust
pub fn analyze(input: &AnalysisInput, config: &Config) -> Report;
```

### 5.2 `pgpreflight-postgres`

Owns:

- parsing with the `sqlparser-rs` PostgreSQL dialect;
- conservative safety validation;
- PostgreSQL connection handling;
- Safe Mode transaction orchestration;
- `EXPLAIN (FORMAT JSON, VERBOSE TRUE)`;
- catalog statistics;
- plan normalization; and
- sanitized error classification.

Representative API:

```rust
let checker = PgPreflight::connect(database_url).await?;
let report = checker.check_sql(sql, &config).await?;
```

The initial implementation uses `tokio-postgres`. TLS must verify the server certificate by default whenever TLS is requested. An insecure no-verification CLI mode is outside v0.1.

### 5.3 `pgpreflight`

The package and binary are both named `pgpreflight`. The CLI owns only arguments, input, configuration discovery, connection URL resolution, rendering, color behavior, redaction, and exit codes. It contains no rule logic.

### 5.4 Dependency direction

```text
pgpreflight -> pgpreflight-postgres -> pgpreflight-core
pgpreflight -----------------------> pgpreflight-core
```

A shared crate with `pgindexlab` is not created until a second concrete consumer proves the need.

## 6. Processing flow

```text
SQL file/stdin
  -> UTF-8 and empty-input validation
  -> sqlparser-rs PostgreSQL AST
  -> Safe SQL validation
  -> PostgreSQL connection
  -> read-only transaction and local timeouts
  -> EXPLAIN JSON and catalog reads
  -> normalized statement, plan, and statistics
  -> rule engine
  -> text or JSON report
```

`sqlparser-rs` is a syntax parser, not the semantic authority. PostgreSQL remains authoritative for name resolution, types, permissions, server syntax, and planning. PostgreSQL-valid syntax that the parser cannot safely understand is rejected as unsupported rather than bypassing validation.

## 7. Accepted and rejected SQL

Accepted outer statements:

- `SELECT`
- `UPDATE`
- `DELETE`

CTEs are accepted only when every data-modifying construct remains within the same policy. A CTE containing an unsupported modifying statement is rejected.

Rejected:

- multiple statements;
- input beginning with `EXPLAIN`;
- `INSERT`, `MERGE`, `COPY`, `CALL`, and `DO`;
- `CREATE`, `ALTER`, `DROP`, and `TRUNCATE`;
- `GRANT`, `REVOKE`, and transaction-control statements;
- `SELECT ... FOR UPDATE`, `FOR SHARE`, `FOR NO KEY UPDATE`, or `FOR KEY SHARE`; and
- any construct whose safety cannot be established from the AST.

Input rules:

- UTF-8 required; UTF-8 BOM accepted and removed;
- invalid UTF-8, empty input, and comment-only input are tool failures;
- a trailing semicolon is accepted;
- exactly one parsed statement is required.

## 8. Safe Mode

The adapter implements the equivalent of:

```sql
BEGIN READ ONLY;
SET LOCAL statement_timeout = '3000ms';
SET LOCAL lock_timeout = '500ms';
EXPLAIN (FORMAT JSON, VERBOSE TRUE) <validated statement>;
-- required catalog reads
ROLLBACK;
```

Required invariants:

- never add `ANALYZE`;
- never send target DML outside the `EXPLAIN` wrapper;
- keep the transaction read-only;
- use `SET LOCAL` for both timeouts;
- roll back on success and every recoverable failure path;
- treat disconnect as server-side rollback and a tool failure;
- execute only required catalog reads in addition to `EXPLAIN`.

Plain `EXPLAIN` plans normal DML; `EXPLAIN ANALYZE` executes it. This is still not an absolute zero-side-effect sandbox. Planner hooks, FDW callbacks, and incorrectly declared immutable functions can behave outside the tool's control.

Documentation must recommend:

- a dedicated least-privilege login with only the privileges needed to plan the intended statements;
- awareness that planning `UPDATE` or `DELETE` may require the corresponding table privilege even in a read-only transaction;
- local, development, staging, or sanitized replica databases; and
- caution before connecting to production.

Default timeouts:

```toml
[postgres]
statement_timeout_ms = 3000
lock_timeout_ms = 500
```

## 9. Connection and redaction

Database URL precedence:

1. `--database-url`
2. `PGPREFLIGHT_DATABASE_URL`
3. `DATABASE_URL`

Documentation prefers environment variables because command-line passwords may appear in shell history or process listings.

Normal output, stderr, and public error formatting must not contain:

- complete SQL text;
- SQL literal values;
- passwords;
- complete credential-bearing URLs; or
- raw driver errors that may contain SQL fragments.

Safe metadata may include statement kind, SQLSTATE, stable failure category, schema/relation names, aliases, estimates, costs, and thresholds.

The raw verbose plan is sensitive transient data because it can contain expressions, literals, or FDW-specific text. It must never be logged, cached, persisted, or included in errors.

## 10. Normalized model

```rust
pub struct AnalysisInput {
    pub statement: StatementFacts,
    pub plan: NormalizedPlan,
    pub relations: Vec<RelationStats>,
}

pub struct StatementFacts {
    pub kind: StatementKind,
    pub target_relation: Option<RelationRef>,
    pub has_where: bool,
    pub has_returning: bool,
    pub join_graph: JoinGraph,
}

pub struct NormalizedPlan {
    pub root: PlanNode,
    pub estimated_affected_rows: Option<f64>,
}

pub struct PlanNode {
    pub kind: PlanNodeKind,
    pub estimated_rows: f64,
    pub startup_cost: f64,
    pub total_cost: f64,
    pub relation: Option<RelationRef>,
    pub relation_alias: Option<String>,
    pub children: Vec<PlanNode>,
}
```

`PlanNodeKind` covers common scan, join, modification, append, gather, limit, and aggregate nodes, with `Other(String)` for unknown node types.

The normalized model does not retain raw `Filter`, `Index Cond`, `Join Filter`, output expressions, SQL text, or literals.

For `UPDATE` and `DELETE`, affected rows are derived from the estimate entering `ModifyTable`, including partitioned/`Append` shapes. If a supported plan shape cannot be interpreted confidently, the value is `None`; no estimate is invented.

Unknown plan nodes do not automatically fail analysis. A dependent rule skips when evidence is insufficient. Structurally invalid JSON or missing mandatory plan fields is an `UnsupportedPlan` tool failure.

## 11. Diagnostic rules

Rules have fixed v0.1 severity, can be enabled/disabled, and use configurable numerical thresholds. Incomplete evidence causes a conservative skip.

Deterministic order:

1. `error` before `warning`;
2. rule ID;
3. relation identity; and
4. plan traversal order.

### PGP001 — UPDATE without WHERE

- Severity: `error`
- Trigger: `UPDATE` with no syntactic `WHERE` clause.
- Fires even when affected-row estimate is unavailable.
- `WHERE TRUE` is considered a present `WHERE` in v0.1.

### PGP002 — DELETE without WHERE

- Severity: `error`
- Trigger: `DELETE` with no syntactic `WHERE` clause.
- Fires even when affected-row estimate is unavailable.
- `WHERE TRUE` is considered a present `WHERE` in v0.1.

### PGP101 — Large affected row set

- Severity: `warning`
- Target: `UPDATE`, `DELETE`

```toml
[rules.PGP101]
enabled = true
max_rows = 10000
max_table_ratio = 0.05
min_rows_for_ratio = 1000
```

Trigger when:

```text
estimated_affected_rows >= max_rows
```

or:

```text
estimated_affected_rows >= min_rows_for_ratio
AND relation row estimate is known
AND affected_rows / relation_rows >= max_table_ratio
```

When relation statistics are unavailable, only the absolute threshold is evaluated.

### PGP102 — Large sequential scan

- Severity: `warning`
- Target: all supported statements

```toml
[rules.PGP102]
enabled = true
min_relation_rows = 100000
max_output_ratio = 0.20
```

For each sequential-scan node, trigger when:

```text
relation_rows >= min_relation_rows
AND scan_output_rows / relation_rows <= max_output_ratio
```

The scan node's `Plan Rows` is its estimated output, not scanned rows. Catalog relation rows approximate the rows scanned by a full sequential scan. The rule skips when statistics are missing/non-positive and does not warn merely because a large relation is scanned. Self-joins are evaluated per plan node and alias.

### PGP103 — Large estimated result set

- Severity: `warning`
- Target: `SELECT`

```toml
[rules.PGP103]
enabled = true
max_result_rows = 100000
```

Trigger when the root plan estimate is at least the threshold. Using the root respects `LIMIT`, aggregation, and upper plan nodes. `UPDATE ... RETURNING` and `DELETE ... RETURNING` are outside this rule in v0.1.

### PGP104 — Possible Cartesian join

- Severity: `warning`
- Target: `SELECT`, `UPDATE ... FROM`, `DELETE ... USING`

A conservative AST join graph is built per relevant query level:

- relation occurrences are vertices;
- supported cross-relation `ON`/`WHERE` predicates add edges;
- `USING` and `NATURAL JOIN` connect operands;
- `CROSS JOIN` and `JOIN ... ON TRUE` add no edge by themselves;
- a later supported predicate can still connect those operands;
- aliases distinguish repeated occurrences.

Trigger when at least two relation occurrences exist and the final provable graph has multiple connected components.

Skip when expression ownership cannot be resolved confidently, including ambiguous unqualified columns, unsupported `LATERAL`, complex correlated subqueries, or unsupported set-returning functions.

## 12. Typed evidence

Each diagnostic carries a typed evidence variant rather than an arbitrary map:

- `missing_where`
- `large_affected_rows`
- `large_sequential_scan`
- `large_result_set`
- `cartesian_join`

Evidence contains the non-sensitive estimates, relation identity, aliases, triggered thresholds, and disconnected relation groups needed by machine consumers. Consumers never parse human messages to understand a diagnostic.

## 13. Configuration

The only supported file name is `pgpreflight.toml`.

Discovery order:

1. exact `--config` path;
2. current directory;
3. parent directories to filesystem root;
4. built-in defaults.

Only the first file found is used; files are not merged.

```toml
version = 1

[postgres]
statement_timeout_ms = 3000
lock_timeout_ms = 500

[rules.PGP001]
enabled = true

[rules.PGP002]
enabled = true

[rules.PGP101]
enabled = true
max_rows = 10000
max_table_ratio = 0.05
min_rows_for_ratio = 1000

[rules.PGP102]
enabled = true
min_relation_rows = 100000
max_output_ratio = 0.20

[rules.PGP103]
enabled = true
max_result_rows = 100000

[rules.PGP104]
enabled = true
```

When a file exists, `version = 1` is required. Unknown keys/rules, unsupported versions, negative rows, non-positive timeouts, non-finite numbers, and ratios outside `0.0..=1.0` are errors. Rule severity is not configurable in v0.1.

## 14. CLI contract

```text
pgpreflight check <INPUT>

Arguments:
  <INPUT>                     SQL file, or - for stdin

Options:
  --database-url <URL>        PostgreSQL connection URL
  --config <PATH>             Explicit configuration file
  --format <FORMAT>           text | json [default: text]
  --fail-on <SEVERITY>        error | warning [default: error]
  -h, --help
  -V, --version
```

No directory traversal, globbing, watch mode, interactive password prompt, or multiple inputs in v0.1.

Text reports go to stdout; text-mode tool failures go to stderr. ANSI color is TTY-only, `NO_COLOR` is honored, and redirected output contains no ANSI escapes.

## 15. JSON contract

Top-level fields:

```json
{
  "schema_version": 1,
  "tool": { "name": "pgpreflight", "version": "0.1.0" },
  "status": "clean",
  "statement": { "kind": "select" },
  "summary": { "errors": 0, "warnings": 0 },
  "diagnostics": [],
  "failure": null
}
```

`status` is `clean`, `warnings`, `errors`, or `failed`.

With `--format json`:

- stdout contains exactly one JSON object for normal clean, diagnostic, and tool-failure outcomes;
- stderr is empty for those outcomes;
- no ANSI escapes are emitted; and
- exit codes still follow `--fail-on`.

The repository stores `schemas/report-v1.schema.json`. While `schema_version` is `1`, existing fields are not removed, their types/meanings do not change, and no new required field is added. Optional fields, rule IDs, and evidence kinds may be added. Consumers must ignore unknown optional additions. Breaking structure requires schema version `2`.

Analysis is atomic: parse, safety, connection, `EXPLAIN`, required catalog, or normalization failure returns `status: "failed"` with no diagnostics. Missing statistics for an individual relation only cause dependent rules to skip; failure to execute the required catalog query is a tool failure.

## 16. Exit codes

| Outcome | Default | `--fail-on warning` |
|---|---:|---:|
| Clean | `0` | `0` |
| Warnings only | `0` | `1` |
| Error diagnostic present | `1` | `1` |
| Input, configuration, connection, planning, or internal failure | `2` | `2` |

JSON status describes the analysis result; exit code describes caller-selected failure policy.

## 17. Error model

Stable categories include:

```text
UnsupportedStatement, MultipleStatements, UnsafeConstruct,
EmptyInput, InvalidUtf8, SqlParse, Configuration,
Connection, Authentication, Timeout, Explain, Catalog,
UnsupportedPlan, Internal
```

Public messages are stable and redacted. SQLSTATE may be exposed. Driver-specific error types do not appear in the public API.

## 18. Testing and CI

### Core tests

Construct `AnalysisInput` without a database. Cover all rule positive/negative cases, threshold boundaries, missing statistics, repeated scans, deterministic ordering, unknown nodes, typed serialization, and report status.

### SQL corpus

```text
tests/sql-corpus/
├── accepted/
├── rejected/
└── known-unsupported/
```

Cover supported statements, forbidden constructs, CTEs, quoted identifiers, `UPDATE ... FROM`, `DELETE ... USING`, joins, subqueries, and known parser gaps.

### PostgreSQL integration

Linux CI runs PostgreSQL 14, 15, 16, 17, and 18. Verify plan acquisition, no target mutation, read-only status, timeouts, rollback, catalog lookup, qualified/quoted names, partitions, version normalization, and affected-row derivation.

Do not snapshot complete plan JSON or exact costs as universal expectations. Assert normalized semantics and evidence.

### CLI/security contracts

Process-level tests verify stdout/stderr, exit codes, JSON Schema, `--fail-on`, stdin/file input, config discovery, color behavior, and secret absence.

Distinctive SQL literals and passwords must be absent from text, JSON, stderr, public error formatting, and normal logs. Raw driver errors never reach renderers.

### CI jobs

```text
format, clippy, unit-tests,
postgres-14, postgres-15, postgres-16, postgres-17, postgres-18,
cross-platform, schema-contract, security-contract, msrv
```

PostgreSQL integration runs on Linux. Linux, macOS, and Windows run builds, unit/parser tests, and DB-independent CLI tests.

The MSRV is selected after locking the initial dependency graph, then declared in workspace metadata, documented, and tested before v0.1.0.

## 19. Compatibility and distribution

v0.1 formally supports PostgreSQL 14 through 18. PostgreSQL 14 reaches community end-of-life on 2026-11-12 but remains in v0.1 because it is supported at design approval and required by the initial maintainer environment. Later support is reevaluated after its final community update.

Formal OS support:

- Linux
- macOS
- Windows

Initial binaries:

- Linux x86_64
- macOS aarch64
- macOS x86_64
- Windows x86_64

Distribution:

- crates.io: `pgpreflight-core`, `pgpreflight-postgres`, `pgpreflight`;
- GitHub Releases with binaries and SHA-256 checksums;
- `cargo install pgpreflight`.

Homebrew, Scoop, and Winget are not v0.1 requirements.

## 20. OSS and security policy

- public repository from inception;
- `MIT OR Apache-2.0` dual license;
- no CLA;
- no telemetry, query upload, crash upload, or automatic update check;
- private vulnerability reporting through GitHub Security Advisories;
- issue templates for bugs, features, false positives, false negatives, and compatibility reports;
- templates warn users to anonymize SQL and schema information;
- `main` protected by PRs, checks, resolved conversations, no force-push, no deletion, and linear history where repository settings permit;
- during `0.x`, only the latest minor line is guaranteed security fixes.

Security reports include credential/literal leakage, unintended execution, Safe Mode bypass, arbitrary code execution, and critical dependency vulnerabilities.

## 21. Documentation and implementation sequence

The implementation phase maintains:

```text
README.md, ARCHITECTURE.md, CHANGELOG.md,
CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md,
docs/REQUIREMENTS.md, docs/API-DESIGN.md, docs/RULES.md,
docs/SAFETY.md, docs/JSON-SCHEMA.md,
docs/COMPATIBILITY.md, docs/ROADMAP.md
```

High-level sequence after specification review:

1. repository/workspace foundation;
2. core models, configuration, report, and JSON Schema;
3. SQL parsing and safety validation;
4. PostgreSQL connection and Safe Mode;
5. plan normalization and catalog statistics;
6. six rules;
7. CLI text/JSON and exit policy;
8. PostgreSQL 14-18 matrix;
9. redaction/security contracts;
10. cross-platform releases;
11. v0.1 documentation and crates.io publication.

Detailed TDD steps and commit boundaries belong in the implementation plan.

## 22. Acceptance criteria

Implementation is complete when:

1. one literal supported statement can be checked against PostgreSQL 14-18;
2. target DML is never intentionally executed and `EXPLAIN ANALYZE` is never used;
3. all six rules meet their evidence and threshold contracts;
4. ambiguous evidence causes a skip rather than a guessed warning;
5. text, JSON, and exit-code contracts pass;
6. secret-leak tests pass;
7. Linux, macOS, and Windows builds pass;
8. PostgreSQL 14-18 integration tests pass; and
9. all three crates pass `cargo publish --dry-run`.

## 23. Authoritative references

- PostgreSQL Versioning Policy: <https://www.postgresql.org/support/versioning/>
- PostgreSQL `EXPLAIN`: <https://www.postgresql.org/docs/current/sql-explain.html>
- PostgreSQL Using `EXPLAIN`: <https://www.postgresql.org/docs/current/using-explain.html>
- PostgreSQL Client Connection Defaults: <https://www.postgresql.org/docs/current/runtime-config-client.html>
- PostgreSQL `SET`: <https://www.postgresql.org/docs/current/sql-set.html>
- `sqlparser-rs`: <https://github.com/apache/datafusion-sqlparser-rs>
