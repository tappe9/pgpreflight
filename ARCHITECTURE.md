# pgpreflight Architecture

Status: **v0.1 design contract; implementation in progress**

This document describes the durable architecture of pgpreflight. It intentionally distinguishes the accepted v0.1 architecture from the subset already implemented on `main`.

## 1. Architectural goals

The architecture prioritizes:

1. **Fail-closed SQL admission.** Unsupported syntax must not bypass the safety gate.
2. **PostgreSQL as semantic authority.** A third-party parser is not used to emulate PostgreSQL planning semantics.
3. **No intentional target-DML execution.** Planning uses plain `EXPLAIN`, never `EXPLAIN ANALYZE`.
4. **Data minimization.** SQL literals, credential-bearing URLs, and raw verbose plans do not enter stable public report models.
5. **Deterministic analysis.** Equivalent normalized evidence and config produce stable diagnostic ordering.
6. **Clear crate boundaries.** Core analysis is independent from PostgreSQL drivers, async runtimes, and CLI rendering.
7. **Testable safety invariants.** Parser validation, adapter behavior, normalization, rules, and rendering are independently testable.

## 2. System boundary

Planned v0.1 pipeline:

```text
input SQL
   │
   ▼
┌──────────────────────────────┐
│ pgpreflight-postgres         │
│ parse + conservative safety  │  [implemented]
└──────────────┬───────────────┘
               │ ValidatedStatement
               ▼
┌──────────────────────────────┐
│ PostgreSQL Safe Mode adapter │  [planned]
│ read-only tx + timeouts      │
│ plain EXPLAIN + catalog      │
└──────────────┬───────────────┘
               │ transient server evidence
               ▼
┌──────────────────────────────┐
│ normalization                │  [planned]
└──────────────┬───────────────┘
               │ AnalysisInput
               ▼
┌──────────────────────────────┐
│ pgpreflight-core             │
│ deterministic rules/report   │  [models/config implemented;
└──────────────┬───────────────┘   rules planned]
               │ Report
        ┌──────┴──────┐
        ▼             ▼
      text           JSON                [CLI planned]
```

`sqlparser-rs` is an admission/safety parser. PostgreSQL remains authoritative for server syntax acceptance, name resolution, types, permissions, statistics, and planning.

## 3. Workspace and dependency direction

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

Allowed dependencies:

```text
pgpreflight -> pgpreflight-postgres -> pgpreflight-core
pgpreflight -----------------------> pgpreflight-core
```

`pgpreflight-core` must not depend on the PostgreSQL client, SQL parser AST, async runtime, terminal concerns, or raw `EXPLAIN` JSON.

## 4. Crate responsibilities

### `pgpreflight-core`

Implemented responsibilities:

- strict versioned configuration types and defaults;
- normalized statement, relation, join-graph, and plan model types;
- typed diagnostic/evidence/report model;
- JSON-serializable report types.

Planned responsibility:

- deterministic PGP001–PGP104 evaluation over normalized evidence.

The core API receives facts; it does not fetch them.

### `pgpreflight-postgres`

Implemented responsibilities:

- PostgreSQL-dialect parsing;
- exactly-one-statement enforcement;
- conservative validation of supported `SELECT`, `UPDATE`, and `DELETE`;
- rejection of unsafe/unsupported query forms;
- sanitized parser/validation errors;
- retention of the validated AST only behind a crate-private boundary for the future adapter.

Planned responsibilities:

- PostgreSQL connection handling;
- read-only transaction orchestration;
- `SET LOCAL` timeout configuration;
- plain `EXPLAIN (FORMAT JSON, VERBOSE TRUE)`;
- catalog statistics reads;
- raw plan normalization and redaction;
- stable adapter error classification.

### `pgpreflight`

The CLI is intentionally thin. Its v0.1 responsibilities are planned to include:

- `check <INPUT>` arguments;
- file/stdin input and UTF-8 handling;
- config discovery;
- database URL precedence;
- text/JSON rendering;
- color policy and `NO_COLOR`;
- `--fail-on` and exit codes.

Rule logic and PostgreSQL-plan interpretation do not belong in the CLI.

## 5. SQL admission boundary

The parser and validator run before database work. They must:

- require exactly one parsed statement;
- allow only the supported statement family;
- recursively reject locking query clauses;
- recursively reject data-modifying query bodies;
- reject `SELECT INTO`;
- reject direct `EXPLAIN`;
- fail closed when a supported safety interpretation cannot be established.

A successful `ValidatedStatement` retains its AST privately so the planning adapter can wrap precisely the statement that passed validation without reparsing a transformed string.

See [docs/SQL-SUPPORT.md](docs/SQL-SUPPORT.md).

## 6. Safe Mode adapter

The accepted v0.1 adapter behavior is equivalent to:

```sql
BEGIN READ ONLY;
SET LOCAL statement_timeout = '3000ms';
SET LOCAL lock_timeout = '500ms';
EXPLAIN (FORMAT JSON, VERBOSE TRUE) <validated statement>;
-- required catalog reads
ROLLBACK;
```

Invariants:

- never add `ANALYZE`;
- never send the target DML outside the `EXPLAIN` wrapper;
- keep the transaction read-only;
- scope timeouts with `SET LOCAL`;
- roll back on normal and recoverable failure paths;
- classify driver failures without exposing raw SQL or credential-bearing URLs.

This adapter is not implemented yet. The contract exists to constrain its implementation.

## 7. Normalization boundary

Raw `EXPLAIN (FORMAT JSON, VERBOSE TRUE)` is sensitive transient evidence. The adapter must transform it into stable core types before rule evaluation.

The normalized model may keep non-sensitive evidence such as:

- statement kind;
- schema/relation identity;
- relation alias;
- plan node kind;
- estimated rows;
- startup/total cost;
- relation row statistics;
- conservative join-graph relationships.

It must not persist raw filters, index conditions, output expressions, SQL text, SQL literals, or complete raw plans merely for convenience.

Unknown plan node kinds should be represented without making the entire report unusable. Rules dependent on evidence that cannot be established should skip rather than invent estimates.

## 8. Rule-engine boundary

`pgpreflight-core` evaluates `AnalysisInput + Config -> Report` deterministically.

Rules must:

- have fixed v0.1 severities;
- use explicit thresholds;
- carry typed evidence rather than requiring consumers to parse human messages;
- skip on insufficient evidence when a warning cannot be justified;
- preserve deterministic ordering.

See [docs/RULES.md](docs/RULES.md).

## 9. Trust and privacy boundaries

Sensitive transient inputs include:

- SQL text and literals;
- database passwords and complete credential-bearing URLs;
- raw PostgreSQL driver errors when they may embed query text;
- raw verbose plans and extension/FDW-specific text.

Stable public errors and reports are designed to contain only classified failures and non-sensitive normalized evidence.

## 10. Testing strategy

Tests are divided by responsibility:

- parser unit/integration tests for exactly-one and fail-closed semantics;
- SQL corpus fixtures for accepted, rejected, and known-unsupported forms;
- core config/report/schema contract tests;
- future adapter integration tests proving read-only planning and non-execution;
- future plan-normalization fixtures using semantic assertions rather than exact cost snapshots;
- future rule boundary tests for every threshold and missing-evidence path;
- future CLI tests for stdout/stderr/exit-code/redaction behavior;
- future PostgreSQL 14–18 integration matrix.

No test should turn an implementation accident into a compatibility promise when PostgreSQL itself is the intended authority.
