# pgpreflight Public API Design

Status: **pre-1.0 v0.1 API; implemented**

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

### `pgpreflight-core` — rule engine implemented

The core crate currently exports:

- `Config`, per-rule configuration structs, and `ConfigError`;
- normalized statement/relation/join/plan types such as `AnalysisInput`, `StatementFacts`, `StatementKind`, `JoinGraph`, `RelationOccurrence`, `NormalizedPlan`, `PlanNode`, and `RelationStats`;
- diagnostic/report types such as `Diagnostic`, `DiagnosticEvidence`, `RuleId`, `Severity`, `Report`, `ReportStatus`, and `FailureInfo`;
- deterministic `analyze(&AnalysisInput, &Config) -> Report` evaluation for PGP001–PGP104.

The crate forbids authored `unsafe` code and deliberately exposes no PostgreSQL client, `sqlparser-rs` AST, raw `EXPLAIN` JSON, or async-runtime types.

`analyze` is database-independent: it consumes normalized evidence and strict configuration, computes typed diagnostics, sorts them deterministically, and derives report status/summary counts.

### `pgpreflight-postgres` — parser and planning API implemented

Current public surfaces include `parse_and_validate`, `ValidatedStatement`, `SafeModePlanner`, `PlannedStatement`, `CheckError`, and `PlanningError`.

`ValidatedStatement` exposes normalized `StatementFacts` through `facts()` but keeps the underlying `sqlparser-rs` `Statement` private to the crate. The planning adapter reuses the exact validated statement without making parser types part of the public compatibility surface.

Validation also constructs the conservative `JoinGraph` used by PGP104. The graph contains safe base-relation identity/alias plus normalized occurrence edges only. When qualified ownership cannot be established, `indeterminate` is set so core analysis skips rather than guesses.

`SafeModePlanner` performs read-only planning with transaction-local timeouts and plain `EXPLAIN`. `PlannedStatement::analysis_input()` exposes only normalized `AnalysisInput`: transient raw plan JSON and expression payloads do not cross the public boundary.

`PlanningError` classifies connection, transaction, configuration, timeout, planning, invalid-plan, catalog, and rollback failures without surfacing raw driver messages.

## 3. Connected CLI facade

The `pgpreflight check <INPUT>` path combines input handling, validation, planning, rule evaluation, and report rendering while keeping its orchestration internals private.

The facade preserves these boundaries:

- validation occurs before planning;
- target DML is only passed to plain `EXPLAIN`;
- raw plan JSON is normalized before crossing into core analysis;
- errors are sanitized before becoming public;
- `pgpreflight-core` remains driver-independent.

## 4. Normalized model policy

Stable models keep only evidence required by rules and machine consumers.

Allowed examples:

- statement kind;
- schema/relation name;
- aliases and relation occurrences;
- syntactic `WHERE` / `RETURNING` facts;
- conservative join-graph edges and `indeterminate` state;
- plan node kind;
- estimated rows/costs;
- normalized relation statistics.

Avoid exposing merely because it is present in a verbose plan or AST:

- complete SQL text;
- literal values;
- raw filter/index/join expressions;
- complete raw plan documents;
- credential-bearing connection data;
- arbitrary driver error payloads.

Unknown plan nodes use the stable `Other(String)` representation. Missing evidence remains missing rather than being guessed.

PGP104 evidence contains deterministic disconnected groups of `RelationOccurrence` and an optional normalized result/affected-row estimate. It does not contain raw SQL or predicate expressions.

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

The typed report model serializes into schema version 1. Machine consumers should inspect structured fields (`rule_id`, `severity`, evidence, status) rather than parse human message text.

See [JSON-REPORT.md](JSON-REPORT.md) and `../schemas/report-v1.schema.json`.

## 7. Error policy

Public error types classify a failure well enough to act on it without embedding raw sensitive inputs.

Good error information includes:

- stable category/kind;
- SQLSTATE when safe and useful;
- non-sensitive relation identity;
- timeout/connectivity category;
- unsupported-plan classification.

Do not expose complete SQL, SQL literals, passwords, complete connection URLs, or raw driver messages that can contain those values.

## 8. Pre-1.0 compatibility

Rust APIs may change before v1.0, but changes should still be intentional and documented. The JSON report uses an explicit schema version because machine consumers need a stronger compatibility boundary than Rust source compatibility during early development.
