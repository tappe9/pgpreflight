# pgpreflight v0.1 Requirements

Status: **accepted v0.1 design contract; implementation in progress**

This document defines the intended v0.1 product contract. It is not a claim that every requirement is already implemented. Current implementation status is summarized in [ROADMAP.md](../ROADMAP.md).

## 1. Product definition

pgpreflight checks one literal PostgreSQL `SELECT`, `UPDATE`, or `DELETE` against a connected PostgreSQL database before application execution.

It combines conservative AST validation, plain `EXPLAIN (FORMAT JSON, VERBOSE TRUE)`, catalog statistics, deterministic diagnostic rules, text output, and versioned JSON output.

## 2. Goals

### G-001 — Real planner evidence
Use the connected PostgreSQL server as the semantic/planning authority rather than attempting to predict plans statically.

### G-002 — Conservative admission
Only send statements to PostgreSQL when the supported safety policy can be established from the parsed AST.

### G-003 — No intentional target-DML execution
Use plain `EXPLAIN`; never intentionally execute the target statement through `EXPLAIN ANALYZE` or an equivalent path.

### G-004 — Actionable deterministic diagnostics
Produce stable rule IDs, severities, thresholds, and typed evidence suitable for humans and automation.

### G-005 — Data minimization
Avoid exposing SQL literals, passwords, complete credential-bearing URLs, and raw verbose plans in stable reports/errors.

### G-006 — Reusable Rust layers
Keep normalized analysis independent from CLI and PostgreSQL-driver implementation details.

## 3. Non-goals for v0.1

v0.1 does not provide:

- SQL execution or `EXPLAIN ANALYZE`;
- measured runtime or exact runtime prediction;
- SQL rewriting, automated fixes, or index creation;
- hypothetical indexes;
- DDL/migration analysis;
- `INSERT`, `MERGE`, `COPY`, `CALL`, or `DO` analysis;
- parameter binding or prepared-statement emulation;
- multi-statement, batch, glob, or directory processing;
- SARIF, editor extensions, or a Web UI;
- telemetry, crash uploads, or query-history storage;
- a universal sandbox for arbitrary functions, extensions, hooks, or FDWs.

## 4. Functional requirements

### FR-001 — Exactly one statement
Input must parse to exactly one statement. Empty/comment-only/invalid input is a tool failure; a trailing semicolon is allowed.

### FR-002 — Supported outer statements
Only `SELECT`, `UPDATE`, and `DELETE` are admitted by v0.1.

### FR-003 — Fail-closed safety validation
Direct `EXPLAIN`, locking queries, `SELECT INTO`, data-modifying nested queries, and unsupported constructs must be rejected before planning.

### FR-004 — PostgreSQL semantic authority
Passing the third-party parser does not imply server validity. PostgreSQL decides name resolution, types, permissions, server syntax acceptance, and plan behavior.

### FR-005 — Safe Mode transaction
The planning adapter must begin a read-only transaction and scope statement/lock timeouts locally to that transaction.

Default timeout contract:

```toml
[postgres]
statement_timeout_ms = 3000
lock_timeout_ms = 500
```

### FR-006 — Plain EXPLAIN only
The adapter must execute the equivalent of:

```sql
EXPLAIN (FORMAT JSON, VERBOSE TRUE) <validated statement>;
```

It must never add `ANALYZE`.

### FR-007 — Catalog evidence
The adapter may execute only the catalog reads required to produce the approved normalized relation evidence.

### FR-008 — Plan normalization
Raw plan JSON must be transformed into stable core plan nodes and estimates. Unknown node kinds must not force invented evidence.

### FR-009 — Deterministic diagnostic rules
v0.1 provides six rules: `PGP001`, `PGP002`, `PGP101`, `PGP102`, `PGP103`, and `PGP104`.

### FR-010 — Strict configuration
`pgpreflight.toml` configuration uses `version = 1`, rejects unknown fields, rejects unsupported versions/invalid thresholds, and starts from built-in defaults.

### FR-011 — Text and versioned JSON
The CLI must support human-readable text and schema-versioned JSON reports.

### FR-012 — JSON Schema v1
Machine output must conform to `schemas/report-v1.schema.json` while `schema_version` is `1`.

### FR-013 — Config discovery
Planned discovery order:

1. explicit `--config` path;
2. current directory;
3. parent directories to filesystem root;
4. built-in defaults.

The first file found is used; files are not merged.

### FR-014 — Database URL precedence
Planned precedence:

1. `--database-url`;
2. `PGPREFLIGHT_DATABASE_URL`;
3. `DATABASE_URL`.

Documentation should prefer environment variables over command-line passwords.

### FR-015 — CLI contract

```text
pgpreflight check <INPUT>

Arguments:
  <INPUT>                     SQL file, or - for stdin

Options:
  --database-url <URL>
  --config <PATH>
  --format <FORMAT>           text | json
  --fail-on <SEVERITY>        error | warning
```

Exact help wording is not a compatibility promise before release, but the semantics above define v0.1 scope.

## 5. Diagnostic requirements

- `PGP001`: `UPDATE` without a syntactic `WHERE` → error.
- `PGP002`: `DELETE` without a syntactic `WHERE` → error.
- `PGP101`: large estimated affected rows → warning.
- `PGP102`: large sequential scan with low estimated output ratio → warning.
- `PGP103`: large estimated `SELECT` result → warning.
- `PGP104`: conservatively provable disconnected join graph → warning.

Rules that require unavailable evidence must skip rather than guess. See [RULES.md](RULES.md).

## 6. Safety and privacy requirements

### SAFE-001 — No ANALYZE
There must be no target-statement `EXPLAIN ANALYZE` path.

### SAFE-002 — Read-only planning
Planning occurs inside a read-only transaction with local timeouts.

### SAFE-003 — Rollback
Normal completion and recoverable failure paths must explicitly roll back; disconnect is treated as a tool failure with PostgreSQL responsible for transaction cleanup.

### SAFE-004 — Secret redaction
Public errors/output must not contain passwords or complete credential-bearing URLs.

### SAFE-005 — SQL minimization
Stable normalized models and public errors must not retain complete SQL text or SQL literal values.

### SAFE-006 — Raw plan minimization
Raw verbose plan JSON is sensitive transient data and must not be logged, cached, persisted, or surfaced as a public failure payload.

### SAFE-007 — No universal sandbox claim
Documentation must state that hooks, FDWs, extensions, and unusual user-defined functions can behave during planning outside pgpreflight's control.

## 7. Compatibility requirements

Target v0.1 support:

- PostgreSQL 14, 15, 16, 17, 18;
- Linux x86_64;
- macOS aarch64/x86_64;
- Windows x86_64;
- Rust 1.85.0 or newer.

The project must distinguish targets from versions/platforms actually covered by CI. See [COMPATIBILITY.md](COMPATIBILITY.md).

## 8. Quality requirements

- authored Rust code should avoid `unsafe`; core and postgres crates currently forbid it;
- parser/validation failures must be typed and sanitized;
- threshold boundaries require explicit tests;
- SQL acceptance/rejection requires permanent regression coverage;
- PostgreSQL integration assertions should test semantics rather than exact plan costs;
- JSON report variants must validate against schema v1;
- public documentation must be updated when safety, SQL policy, rules, schema, or compatibility changes.

## 9. Current implementation checkpoint

Implemented on `main` at the time this document was introduced:

- workspace/MSRV/license/CI foundation;
- strict core config and defaults;
- normalized model and report types;
- report JSON Schema v1;
- parser and conservative validation for the current supported SQL surface.

Safe Mode, normalization, rule evaluation, CLI, PostgreSQL compatibility matrix, and release packaging remain future v0.1 slices.
