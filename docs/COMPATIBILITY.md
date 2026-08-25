# Compatibility

Status: **v0.1 target with partial CI verification**

This document separates intended support from what the repository currently verifies automatically.

## Rust

Workspace metadata currently declares:

- edition: Rust 2024;
- minimum Rust version: **1.85**.

Current GitHub Actions install Rust 1.85.0 for quality and cross-platform jobs.

Raising the MSRV is a compatibility change and should be explicit rather than occurring accidentally through a dependency update.

## Operating systems

### Current CI verification

- Ubuntu: formatting, Clippy, and full workspace tests with all features;
- macOS: workspace/all-targets `cargo check`;
- Windows: workspace/all-targets `cargo check`.

### v0.1 target

- Linux x86_64;
- macOS x86_64 and aarch64;
- Windows x86_64.

The current cross-platform jobs prove compilation on GitHub-hosted macOS/Windows runners; they do not yet represent final release-artifact coverage.

## PostgreSQL

v0.1 target: PostgreSQL **14, 15, 16, 17, and 18**.

This is currently a **target, not a completed compatibility claim**. The PostgreSQL planning adapter has not landed yet, so there is no server-version integration matrix on `main` today.

Before v0.1 release, integration CI should exercise every targeted major version with semantic assertions covering:

- read-only Safe Mode;
- plain `EXPLAIN` behavior;
- representative plan normalization;
- catalog-statistics access;
- sanitized failure behavior;
- supported SQL planning.

Tests should avoid exact cost snapshots because planner costs/statistics can vary legitimately between versions and environments.

## SQL parser compatibility

`sqlparser-rs` provides the local PostgreSQL-dialect AST used by the conservative safety gate. Its accepted syntax is **not** pgpreflight's PostgreSQL compatibility promise.

A statement may be:

- valid PostgreSQL but unsupported by the local parser/policy;
- parsed by the local parser but rejected by pgpreflight's safety policy;
- admitted locally but later rejected by the connected PostgreSQL server.

This distinction is intentional. See [SQL-SUPPORT.md](SQL-SUPPORT.md).

## JSON compatibility

Machine report compatibility is versioned separately through `schema_version`. See [JSON-REPORT.md](JSON-REPORT.md).

## Pre-1.0 Rust API policy

Public Rust APIs may evolve before v1.0. Changes should nevertheless be deliberate, documented, and avoid leaking implementation-specific parser/driver types into stable core models.

## Release packaging

No v0.1.0 release artifacts are published yet. crates.io metadata, dry-run publication validation, tagged binary artifacts, and checksums belong to the release-readiness milestone.
