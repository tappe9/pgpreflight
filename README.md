# pgpreflight

[![CI](https://github.com/tappe9/pgpreflight/actions/workflows/ci.yml/badge.svg)](https://github.com/tappe9/pgpreflight/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

`pgpreflight` is a Rust CLI and library project for checking one literal PostgreSQL `SELECT`, `UPDATE`, or `DELETE` statement against PostgreSQL's real planner before an application executes it.

The v0.1 design combines conservative SQL validation, plain `EXPLAIN (FORMAT JSON, VERBOSE TRUE)`, catalog statistics, deterministic diagnostics, and versioned machine-readable reports. It is intentionally a **preflight tool**, not a SQL executor or an `EXPLAIN ANALYZE` wrapper.

> **Project status:** pre-release and under active development. The workspace foundation, core configuration/report contracts, JSON Schema v1, and conservative SQL parsing/validation are implemented on `main`. PostgreSQL Safe Mode planning, plan normalization, diagnostic evaluation, and the end-user `check` CLI are still v0.1 work in progress.

日本語版: [README.ja.md](README.ja.md)

## Why pgpreflight

A query can be syntactically valid and still be risky to run: an `UPDATE` can omit `WHERE`, a plan can imply a broad sequential scan, or a result can be unexpectedly large. Static SQL inspection alone cannot reproduce PostgreSQL's name resolution, permissions, statistics, planner choices, or server-version behavior.

`pgpreflight` therefore uses two authorities with different responsibilities:

1. a conservative PostgreSQL-dialect AST validator decides whether pgpreflight is willing to inspect the statement at all;
2. PostgreSQL itself is the semantic and planning authority for statements that pass that gate.

When the validator cannot establish that a construct fits the supported safety policy, it fails closed rather than sending it to PostgreSQL.

## Current implementation on `main`

Implemented today:

- a Rust 2024 Cargo workspace with `pgpreflight-core`, `pgpreflight-postgres`, and `pgpreflight`;
- strict versioned core configuration and the approved v0.1 default thresholds;
- normalized public model and diagnostic/report types without PostgreSQL-driver or parser-AST types in the core API;
- `schemas/report-v1.schema.json` for the versioned JSON report contract;
- PostgreSQL-dialect parsing through `sqlparser-rs`;
- exactly-one-statement validation;
- conservative acceptance of `SELECT`, `UPDATE`, and `DELETE`;
- rejection of direct `EXPLAIN`, locking queries, `SELECT INTO`, data-modifying nested queries, and unsupported statement forms;
- accepted/rejected/known-unsupported SQL corpus fixtures;
- Linux quality CI and macOS/Windows cross-platform checks using Rust 1.85.0.

Not yet implemented as an end-to-end product:

- PostgreSQL connection and Safe Mode transaction orchestration;
- `EXPLAIN` execution and catalog reads;
- plan/statistics normalization;
- PGP001–PGP104 rule evaluation;
- `pgpreflight check <INPUT>` rendering, config discovery, and exit-code behavior;
- PostgreSQL 14–18 integration matrix and release packaging.

See [ROADMAP.md](ROADMAP.md) for implementation order and status.

## Planned v0.1 flow

```text
SQL file / stdin
      │
      ▼
UTF-8 + single-statement validation
      │
      ▼
conservative PostgreSQL AST safety gate
      │
      ▼
PostgreSQL connection
      │
      ▼
read-only transaction + local timeouts
      │
      ▼
EXPLAIN (FORMAT JSON, VERBOSE TRUE)
      │
      ├── catalog statistics
      ▼
normalized statement / plan / relation facts
      │
      ▼
deterministic PGP001–PGP104 rules
      │
      ├── text report
      └── JSON report schema v1
```

The future planning path must use plain `EXPLAIN`, **never `EXPLAIN ANALYZE`**.

## SQL policy

v0.1 is deliberately narrow:

- exactly one statement;
- outer statement must be `SELECT`, `UPDATE`, or `DELETE`;
- direct `EXPLAIN` input is rejected;
- locking clauses are rejected;
- `SELECT INTO` is rejected;
- data-modifying nested queries/CTEs are rejected;
- unsupported or ambiguous forms fail closed;
- parameter placeholders are outside the v0.1 contract; the intended workflow uses one statement with literal values.

The parser is a safety gate, not PostgreSQL's semantic replacement. See [SQL support](docs/SQL-SUPPORT.md).

## Diagnostics planned for v0.1

| Rule | Severity | Purpose |
| --- | --- | --- |
| `PGP001` | error | `UPDATE` without `WHERE` |
| `PGP002` | error | `DELETE` without `WHERE` |
| `PGP101` | warning | large estimated affected row set |
| `PGP102` | warning | large sequential scan with low estimated output ratio |
| `PGP103` | warning | large estimated `SELECT` result set |
| `PGP104` | warning | conservatively provable Cartesian join risk |

Configuration types and defaults already exist in `pgpreflight-core`; the rule engine itself is still planned work. See [Rules](docs/RULES.md).

## Safety model

The v0.1 design adds multiple independent controls:

- conservative AST validation before database access;
- a read-only transaction;
- local statement and lock timeouts;
- plain `EXPLAIN` only;
- no intentional execution of target DML;
- sanitized errors and report models that do not retain SQL text or credentials;
- least-privilege connection guidance.

These controls are **not a universal PostgreSQL sandbox**. Planner hooks, FDWs, extensions, and incorrectly declared user-defined functions may perform behavior outside pgpreflight's control during planning. Production access therefore requires the same care as any other database tooling.

See [Safety model](docs/SAFETY.md) and [Security Policy](SECURITY.md).

## Workspace

```text
pgpreflight/
├── crates/
│   ├── pgpreflight-core/      # normalized models, config, diagnostics, reports
│   ├── pgpreflight-postgres/  # PostgreSQL parser/safety/planning adapter
│   └── pgpreflight/           # end-user CLI
├── docs/
├── schemas/
└── tests/
```

Dependency direction is intentionally one-way:

```text
pgpreflight -> pgpreflight-postgres -> pgpreflight-core
pgpreflight -----------------------> pgpreflight-core
```

## Library surface currently available

The PostgreSQL crate currently exposes the conservative validation entry point:

```rust
use pgpreflight_postgres::parse_and_validate;

let validated = parse_and_validate("UPDATE public.accounts SET active = false WHERE id = 42")?;
println!("{:?}", validated.facts().kind);
# Ok::<(), pgpreflight_postgres::CheckError>(())
```

The validated object deliberately does not expose `sqlparser-rs` AST types as public API. The connected planning facade described in the API design is a v0.1 target, not an implemented API yet.

## Compatibility target

- Rust: **1.85.0+** (`edition = 2024`)
- PostgreSQL: **14–18 target**; database integration coverage is not complete yet
- OS target: Linux x86_64, macOS aarch64/x86_64, Windows x86_64

Current CI coverage and target-vs-verified distinctions are documented in [Compatibility](docs/COMPATIBILITY.md).

## Documentation

- [Requirements](docs/REQUIREMENTS.md)
- [Architecture](ARCHITECTURE.md)
- [Public API design](docs/API-DESIGN.md)
- [SQL support and validation policy](docs/SQL-SUPPORT.md)
- [Safety model](docs/SAFETY.md)
- [Diagnostic rules](docs/RULES.md)
- [JSON report contract](docs/JSON-REPORT.md)
- [Compatibility](docs/COMPATIBILITY.md)
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Security Policy](SECURITY.md)

Development-agent scratch plans under `docs/superpowers/` are intentionally not project documentation and are ignored by Git.

## Non-goals for v0.1

pgpreflight v0.1 does not aim to provide:

- SQL execution or `EXPLAIN ANALYZE`;
- exact runtime prediction;
- SQL rewriting, automatic fixes, or index creation;
- hypothetical indexes;
- DDL/migration analysis;
- `INSERT`, `MERGE`, `COPY`, `CALL`, or `DO` analysis;
- parameter binding or prepared-statement emulation;
- batch/glob/directory processing;
- telemetry, crash uploads, or query-history storage;
- a universal sandbox for arbitrary PostgreSQL extensions or functions.

## Contributing

The project is developed in public with small, testable implementation slices. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)); or
- MIT License ([LICENSE-MIT](LICENSE-MIT)).

You may choose either license when using or redistributing pgpreflight.
