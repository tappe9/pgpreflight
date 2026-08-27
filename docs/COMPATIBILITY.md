# Compatibility

Status: **v0.1 compatibility and release-packaging contracts verified in CI**

This document separates source/test compatibility from release-artifact support.

## Rust

Workspace metadata declares:

- edition: Rust 2024;
- minimum Rust version: **1.85**.

The dedicated `msrv` job installs Rust 1.85.0 and runs:

```bash
cargo +1.85.0 check --workspace --all-targets --all-features
```

Formatting, Clippy, and the non-database platform matrix use the current stable toolchain separately. This keeps the declared MSRV explicit while still detecting incompatibilities with current Rust tooling.

Raising the MSRV is a compatibility change and should be explicit rather than occurring accidentally through a dependency update.

## Operating systems

### Source and non-database CI verification

The `cross-platform` matrix runs on GitHub-hosted:

- Ubuntu;
- macOS;
- Windows.

Every platform entry performs both:

```bash
cargo +stable build --workspace --all-targets --all-features
cargo +stable test --workspace --all-features
```

Database-backed tests skip themselves when `PGPREFLIGHT_TEST_DATABASE_URL` is absent, so this matrix fixes build and non-database behavior across all three operating-system families.

### v0.1 release-artifact target

- Linux x86_64;
- macOS x86_64 and aarch64;
- Windows x86_64.

The tagged release workflow builds archives for all four targets and publishes per-archive SHA-256 sidecars plus a combined `SHA256SUMS` manifest. No v0.1.0 tag or release artifacts have been published yet.

## PostgreSQL

v0.1 supports PostgreSQL **14, 15, 16, 17, and 18** through a dedicated server matrix.

Each matrix entry:

- boots the matching PostgreSQL major version;
- exposes the expected major through `PGPREFLIGHT_TEST_POSTGRES_MAJOR`;
- verifies the actual server major from `server_version_num`;
- runs the complete workspace test suite with database integration enabled.

The PostgreSQL-backed assertions are semantic rather than snapshot-based. Coverage verifies the Safe Mode transaction, target-DML non-execution, representative normalized plan kinds and relation identity, conservative affected-row evidence, catalog statistics, supported SQL planning, and sanitized failures. It deliberately does not lock exact startup cost, total cost, row-estimate, or page-count snapshots that may vary legitimately between PostgreSQL versions or environments.

## Stable CI checks

The workflow exposes stable leaf job names for diagnosis:

- `CI / quality`;
- `CI / msrv`;
- `CI / cross-platform / non-db (<runner>)`;
- `CI / postgresql / <major>`;
- `CI / release-readiness`.

Branch protection should require the stable aggregate check **`CI / required`**. That job depends on every leaf matrix and fails unless all quality, MSRV, platform, PostgreSQL, and release-readiness jobs succeed. Requiring the aggregate avoids changing branch-protection settings whenever a matrix entry is added or renamed.

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

The three crates include crates.io metadata, per-crate README and license files, and committed package contents. CI validates locked metadata, performs a full `pgpreflight-core` publish dry-run, and prepares the two dependent packages with temporary local registry patches. The tagged release workflow builds the four supported native archives and SHA-256 checksums. No v0.1.0 crates, tag, or release artifacts have been published yet.
