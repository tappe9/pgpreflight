# pgpreflight Public API Design

Status: **pre-1.0 v0.1 design; partially implemented**

This document defines durable public-API boundaries. It separates types/functions available on `main` from APIs planned for later v0.1 slices.

## 1. Design principles

The public API should:

- expose normalized, non-sensitive facts rather than PostgreSQL-driver or parser internals;
- keep `pgpreflight-core` deterministic and database-independent;
- use typed errors and typed diagnostic evidence;
- make unsupported/unknown evidence explicit rather than inventing values;
- keep SQL text and credential-bearing connection details out of stable report models;
- preserve room for the pre-1.0 API to evolve intentionally.

## 2. Crate surface

### `pgpreflight-core` — implemented

The core crate currently exports:

- `Config`, per-rule configuration structs, and `ConfigError`;
- normalized statement/relation/join/plan types such as `AnalysisInput`, `StatementFacts`, `StatementKind`, `NormalizedPlan`, `PlanNode`, and `RelationStats`;
- diagnostic/report types such as `Diagnostic`, `DiagnosticEvidence`, `RuleId`, `Severity`, `Report`, `ReportStatus`, and `FailureInfo`.

The crate forbids authored `unsafe` code and deliberately exposes no PostgreSQL client, `sqlparser-rs` AST, raw `EXPLAIN` JSON, or async-runtime types.

A representative future analysis entry point is conceptually:

```rust
pub fn analyze(input: &AnalysisInput, config: &Config) -> Report;
```

The model/config/report contracts exist today; the complete rule-evaluation facade is still planned.

### `pgpreflight-postgres` — parser API implemented

Current public entry point:

```rust
pub fn parse_and_validate(sql: &str) -> Result<ValidatedStatement, CheckError>;
```

`ValidatedStatement` exposes normalized `StatementFacts` through `facts()` but keeps the underlying `sqlparser-rs` `Statement` private to the crate. This allows the future planning adapter to reuse the exact validated AST without making parser types part of the public compatibility surface.

`CheckError` currently classifies:

- SQL parse failure;
- multiple statements;
- unsupported statement type/form;
- explicitly unsafe constructs with a stable non-sensitive kind label.

Errors intentionally do not echo the original SQL.

## 3. Planned connected facade

A future v0.1 facade may have a shape equivalent in purpose to:

```rust
let checker = PgPreflight::connect(database_url).await?;
let report = checker.check_sql(sql, &config).await?;
```

This is a **design direction, not an implemented API commitment**. Exact names/signatures may change before the adapter issue lands.

Whatever facade is chosen must preserve these boundaries:

- validation occurs before planning;
- target DML is only passed to plain `EXPLAIN`;
- raw plan JSON is normalized before crossing into core analysis;
- errors are sanitized before becoming public;
- `pgpreflight-core` remains driver-independent.

## 4. Normalized model policy

Stable models should keep only evidence required by rules and machine consumers.

Allowed examples:

- statement kind;
- schema/relation name;
- aliases;
- syntactic `WHERE` / `RETURNING` facts;
- plan node kind;
- estimated rows/costs;
- normalized relation statistics;
- conservative join-graph relationships.

Avoid exposing merely because it is present in a verbose plan:

- complete SQL text;
- literal values;
- raw filter/index/join expressions;
- complete raw plan documents;
- credential-bearing connection data;
- arbitrary driver error payloads.

Unknown plan nodes may use a stable `Other(String)` representation. Missing evidence should remain missing rather than being guessed.

## 5. Configuration API

`Config` is versioned and uses strict Serde deserialization with unknown-field rejection.

Current defaults include:

```toml
version = 1

[postgres]
statement_timeout_ms = 3000
lock_timeout_ms = 500

[rules.PGP101]
max_rows = 10000
max_table_ratio = 0.05
min_rows_for_ratio = 1000

[rules.PGP102]
min_relation_rows = 100000
max_output_ratio = 0.20

[rules.PGP103]
max_result_rows = 100000
```

All six v0.1 rules default to enabled. Ratios outside `0.0..=1.0`, negative row thresholds, and unsupported config versions are invalid.

## 6. Report API and JSON

The typed report model is designed to serialize into schema version 1. Machine consumers should inspect structured fields (`rule_id`, `severity`, evidence, status) rather than parse human message text.

See [JSON-REPORT.md](JSON-REPORT.md) and `../schemas/report-v1.schema.json`.

## 7. Error policy

Public error types should classify a failure well enough to act on it without embedding raw sensitive inputs.

Good error information includes:

- stable category/kind;
- SQLSTATE when safe and useful;
- non-sensitive relation identity;
- timeout/connectivity category;
- unsupported-plan classification.

Do not expose complete SQL, SQL literals, passwords, complete connection URLs, or raw driver messages that can contain those values.

## 8. Pre-1.0 compatibility

Rust APIs may change before v1.0, but changes should still be intentional and documented. The JSON report uses an explicit schema version because machine consumers need a stronger compatibility boundary than Rust source compatibility during early development.
