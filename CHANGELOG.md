# Changelog

All notable changes to this project are documented here. The format is based on Keep a Changelog and the project follows Semantic Versioning.

## [Unreleased]

## [0.1.0] - 2026-08-27

### Added

- Conservative validation for one literal PostgreSQL `SELECT`, `UPDATE`, or `DELETE`.
- Read-only Safe Mode planning with plain `EXPLAIN` and bounded local timeouts.
- Normalized plan/statistics evidence and deterministic PGP001–PGP104 diagnostics.
- Text and schema-v1 JSON output from `pgpreflight check`.
- PostgreSQL 14–18, Rust 1.85, Linux, macOS, and Windows CI coverage.

[Unreleased]: https://github.com/tappe9/pgpreflight/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tappe9/pgpreflight/releases/tag/v0.1.0
