# pgpreflight

Preflight PostgreSQL queries with the real planner—without intentionally executing the target DML.

> **Project status:** design approved; implementation planning has not started.

`pgpreflight` will be an open-source Rust CLI and library for checking one literal `SELECT`, `UPDATE`, or `DELETE` statement against a connected PostgreSQL database. It will combine conservative SQL validation, plain `EXPLAIN (FORMAT JSON, VERBOSE TRUE)`, catalog statistics, and deterministic diagnostics.

The approved v0.1 design is documented in:

- [`docs/superpowers/specs/2026-08-19-pgpreflight-design.md`](docs/superpowers/specs/2026-08-19-pgpreflight-design.md)

## Planned v0.1 diagnostics

- `PGP001` — `UPDATE` without `WHERE`
- `PGP002` — `DELETE` without `WHERE`
- `PGP101` — large affected row set
- `PGP102` — large sequential scan
- `PGP103` — large estimated result set
- `PGP104` — possible Cartesian join

## Safety direction

The tool will use plain `EXPLAIN`, never `EXPLAIN ANALYZE`, inside a read-only transaction with local timeouts. This is an additional guard rather than a universal sandbox for arbitrary user-defined functions, extensions, or FDWs.

## Compatibility target

- PostgreSQL 14–18
- Linux
- macOS
- Windows

## License

The project is planned to use `MIT OR Apache-2.0` licensing. License files will be added with the implementation foundation.
