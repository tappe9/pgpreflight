# pgpreflight Roadmap

This roadmap is directional. GitHub Issues are the execution tracker; this file explains milestone boundaries and current implementation status.

## v0.1 — Planner-backed preflight MVP

Status: **release-ready on `main`; not released**.

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
- [x] **Issue #9 — `pgpreflight check` CLI**
  - SQL file and stdin input with BOM, UTF-8, empty/comment-only, and exactly-one-statement handling;
  - strict single-file config discovery and database URL precedence;
  - human-readable text and schema-v1 JSON output;
  - `--fail-on error|warning` and fixed clean/diagnostic/tool-failure exit codes;
  - sanitized parser/driver/tool failures without SQL literal or credential-bearing URL leakage;
  - process integration coverage for streams, exit codes, resolution precedence, redaction, and connected non-execution.
- [x] **Issue #10 — compatibility and safety matrix**
  - PostgreSQL 14–18 matrix with actual-major checks and semantic Safe Mode/normalization/non-execution assertions;
  - Linux/macOS/Windows all-target build and non-database test matrix;
  - dedicated Rust 1.85.0 MSRV job;
  - schema-v1 validation for clean, warning, error-diagnostic, and tool-failure reports;
  - explicit SQL-literal, credential-bearing URL, raw parser-error, and raw driver-error leak regressions across streams and public formatting;
  - stable `CI / required` aggregate check for branch protection.

### Release readiness

- [x] **Issue #11 — v0.1 OSS release readiness**
  - crates.io metadata and package contents for all three crates;
  - dependency-ordered package preflight, with a core publish dry-run before the first release;
  - four-target native release workflow and SHA-256 checksums;
  - final English/Japanese user and developer documentation.

## v0.1 release criteria

Before a v0.1.0 tag, the project should demonstrate:

- [x] no intentional `EXPLAIN ANALYZE` or target-DML execution path;
- [x] read-only Safe Mode behavior covered by integration tests;
- [x] PGP001–PGP104 semantics and threshold boundaries covered by tests;
- [x] JSON schema v1 validation for clean, warning, error-diagnostic, and tool-failure reports;
- [x] PostgreSQL 14–18 semantic integration coverage;
- [x] Linux/macOS/Windows and MSRV checks;
- [x] explicit credential/SQL-literal/parser/driver leak regression coverage;
- [x] publish/release packaging checks;
- [x] README and final release documentation aligned with packaged behavior.

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
