# pgpreflight

Preflight PostgreSQL queries with the real planner—without intentionally executing the target DML.

> **Project status:** implementation has begun; the v0.1 workspace foundation is in place and feature work is tracked through GitHub Issues.

`pgpreflight` is an open-source Rust CLI and library for checking one literal `SELECT`, `UPDATE`, or `DELETE` statement against a connected PostgreSQL database. It will combine conservative SQL validation, plain `EXPLAIN (FORMAT JSON, VERBOSE TRUE)`, catalog statistics, and deterministic diagnostics.

The approved v0.1 design and implementation plan are documented in:

- [`docs/superpowers/specs/2026-08-19-pgpreflight-design.md`](docs/superpowers/specs/2026-08-19-pgpreflight-design.md)
- [`docs/superpowers/plans/2026-08-19-pgpreflight-v0.1.md`](docs/superpowers/plans/2026-08-19-pgpreflight-v0.1.md)

## Workspace

The project is a Cargo workspace with three crates:

- `pgpreflight-core` — normalized models, configuration, diagnostics, rules, and stable report types.
- `pgpreflight-postgres` — PostgreSQL-specific parsing, safety validation, planning, catalog access, and normalization.
- `pgpreflight` — the end-user CLI binary.

The dependency direction is intentionally one-way:

```text
pgpreflight -> pgpreflight-postgres -> pgpreflight-core
pgpreflight -----------------------> pgpreflight-core
```

## Planned v0.1 diagnostics

- `PGP001` — `UPDATE` without `WHERE`
- `PGP002` — `DELETE` without `WHERE`
- `PGP101` — large affected row set
- `PGP102` — large sequential scan
- `PGP103` — large estimated result set
- `PGP104` — possible Cartesian join

## Safety direction

The tool will use plain `EXPLAIN`, never `EXPLAIN ANALYZE`, inside a read-only transaction with local timeouts. This is an additional guard rather than a universal sandbox: user-defined functions, extensions, planner hooks, and FDWs can have behavior outside `pgpreflight`'s control.

## Compatibility target

- PostgreSQL 14–18
- Linux x86_64
- macOS aarch64 / x86_64
- Windows x86_64
- Rust 1.85 or newer

## License

Licensed under either of:

- Apache License, Version 2.0 (`LICENSE-APACHE`)
- MIT License (`LICENSE-MIT`)

at your option.
