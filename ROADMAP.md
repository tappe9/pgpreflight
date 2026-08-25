# pgpreflight Roadmap

This roadmap is directional. GitHub Issues are the execution tracker; this file explains milestone boundaries and current implementation status.

## v0.1 — Planner-backed preflight MVP

Status: **in progress on `main`; not released**.

Goal: inspect one literal `SELECT`, `UPDATE`, or `DELETE` using a conservative safety gate plus PostgreSQL's real planner, then emit deterministic text/JSON diagnostics without intentionally executing the target DML.

### Completed foundations

- [x] **Issue #1 — workspace and OSS foundation**
  - three-crate Rust 2024 workspace;
  - Rust 1.85.0 baseline;
  - MIT OR Apache-2.0 licensing;
  - baseline Linux/macOS/Windows CI.
- [x] **Issue #2 — core contracts and JSON schema v1**
  - normalized core model;
  - strict versioned config/defaults;
  - diagnostic/report types;
  - `schemas/report-v1.schema.json`.
- [x] **Issue #3 — parse and safely validate SELECT/UPDATE/DELETE**
  - PostgreSQL-dialect parser;
  - exactly-one-statement contract;
  - fail-closed supported-statement policy;
  - nested modification/locking/`SELECT INTO` rejection;
  - accepted/rejected/known-unsupported SQL corpus.
- [x] **Issue #4 — PostgreSQL Safe Mode planning adapter**
  - read-only transaction orchestration;
  - transaction-local statement/lock timeouts;
  - plain `EXPLAIN (FORMAT JSON, VERBOSE TRUE)` only;
  - rollback and sanitized adapter failures.
- [x] **Issue #5 — plan and relation-statistics normalization**
  - stable normalized plan nodes with unknown-node preservation;
  - conservative UPDATE/DELETE affected-row estimates;
  - PostgreSQL catalog relation statistics without fabricated missing values;
  - raw expression/literal data excluded from normalized evidence.
- [x] **Issue #6 — PGP001/PGP002/PGP101**
  - errors for `UPDATE`/`DELETE` without a syntactic `WHERE`, with `WHERE TRUE` treated as present;
  - large affected-row warning by absolute rows and/or relation ratio with exact inclusive boundaries;
  - missing relation statistics preserved as unknown so ratio evaluation is skipped rather than guessed;
  - deterministic diagnostic ordering, status, and summary counts.
- [x] **Issue #7 — PGP102/PGP103**
  - per-node large sequential-scan warnings using inclusive relation-size and output-ratio thresholds;
  - missing/non-positive statistics and non-sequential scans skipped without invented evidence;
  - self-join sequential scans evaluated independently;
  - large `SELECT` result warnings based on normalized root estimated rows so upper nodes such as `LIMIT` naturally apply;
  - `UPDATE`/`DELETE ... RETURNING` excluded from PGP103;
  - exact boundaries, deterministic ordering/counts, and PostgreSQL-backed normalized-evidence integration covered by tests.
- [x] **Issue #8 — PGP104**
  - conservative relation-occurrence graphs derived from the validated AST;
  - qualified `WHERE`/`ON` predicates, `USING`, and `NATURAL JOIN` used as provable edges;
  - `CROSS JOIN`/`ON TRUE` left disconnected unless later supported predicates connect the groups;
  - aliased self joins represented as distinct occurrences;
  - ambiguous, unqualified, lateral, correlated, and unsupported ownership marked indeterminate and skipped;
  - deterministic connected components, diagnostic ordering/counts, safe evidence, and supported join-bearing `UPDATE`/`DELETE` covered by tests.

### Remaining v0.1 implementation order

1. **Issue #9 — `pgpreflight check` CLI**  
   Input/config/connection resolution, text/JSON output, `--fail-on`, exit codes, redaction.
2. **Issue #10 — compatibility and safety matrix**  
   PostgreSQL 14–18 integration coverage, platform/MSRV checks, secret-leak regressions, schema coverage.
3. **Issue #11 — v0.1 OSS release readiness**  
   packaging metadata, release workflow, final user/developer documentation, dry-run publication checks.

## v0.1 release criteria

Before a v0.1.0 tag, the project should demonstrate:

- no intentional `EXPLAIN ANALYZE` or target-DML execution path;
- read-only Safe Mode behavior covered by integration tests;
- PGP001–PGP104 semantics and threshold boundaries covered by tests;
- JSON schema v1 validation for clean, diagnostic, and tool-failure reports;
- PostgreSQL 14–18 semantic integration coverage;
- Linux/macOS/Windows and MSRV checks;
- explicit credential/SQL-literal leak regression coverage;
- publish/release packaging checks;
- README, safety, rules, API, JSON, compatibility, contributing, and security docs aligned with implementation.

## After v0.1

Candidates intentionally deferred from the first release include:

- parameter binding / prepared-statement-aware workflows;
- more statement kinds;
- richer plan diagnostics;
- additional machine-report formats such as SARIF;
- editor integrations;
- batch analysis;
- hypothetical-index exploration in a separate project rather than silently expanding pgpreflight's scope.

These are candidates, not commitments. v0.1 should remain small enough that its safety boundary is understandable and testable.

## Guiding rule

Prefer independently useful, testable slices. Do not weaken the conservative SQL gate or blur the distinction between planning and execution merely to accept more PostgreSQL syntax.
