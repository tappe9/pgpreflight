# Contributing to pgpreflight

Thank you for considering a contribution.

pgpreflight is pre-1.0 software with a deliberately conservative PostgreSQL safety boundary. Correctness, fail-closed behavior, and data minimization matter more than accepting the widest possible SQL surface.

## Development principles

Contributions should preserve these rules:

- keep `pgpreflight-core` independent from PostgreSQL clients, parser AST types, async runtimes, and CLI concerns;
- treat `sqlparser-rs` as a safety/admission parser, not PostgreSQL's semantic authority;
- reject unsupported or ambiguous SQL rather than bypassing validation;
- never introduce an `EXPLAIN ANALYZE` path for target statements;
- keep target DML inside the intended plain-`EXPLAIN` planning boundary;
- do not retain SQL literals, complete credential-bearing URLs, or raw verbose plans in stable public models;
- add focused tests before or with behavior changes;
- prefer small PRs with one responsibility and clear acceptance criteria.

## TDD workflow

Behavior changes should follow Red → Green → Refactor:

1. add the smallest test or corpus fixture that defines the required behavior;
2. run it and confirm the expected failure;
3. implement the minimum behavior;
4. run the focused test to green;
5. refactor without changing the contract;
6. run the complete relevant quality gates.

For a bug discovered by integration or fuzz-style testing, first preserve a deterministic regression before fixing it.

## SQL-policy changes

A PR that changes accepted/rejected SQL should explain:

- the statement/query shape;
- why it is safe to admit or why it must be rejected;
- how nested queries/CTEs are handled;
- whether the change affects `StatementFacts`;
- corpus or unit tests covering the decision;
- documentation updates in `docs/SQL-SUPPORT.md` when the public policy changes.

Do not accept a construct solely because `sqlparser-rs` can parse it. pgpreflight must be able to justify the safety policy for the entire relevant query shape.

## PostgreSQL adapter changes

Adapter work should preserve the Safe Mode contract in [docs/SAFETY.md](docs/SAFETY.md). Integration tests should prove semantic invariants such as read-only state and non-execution rather than depending on exact planner cost numbers.

Version-specific PostgreSQL behavior must be checked against the PostgreSQL 14–18 matrix. Add semantic assertions that tolerate legitimate planner estimate/cost differences, and set `PGPREFLIGHT_TEST_POSTGRES_MAJOR` when reproducing a particular matrix entry locally.

## Public API and report changes

Before v1.0 the Rust API may evolve, but breaking changes should still be deliberate.

When changing public models:

- keep driver/parser internals out of `pgpreflight-core` public types;
- preserve typed evidence instead of forcing consumers to parse human messages;
- review `schemas/report-v1.schema.json` and [docs/JSON-REPORT.md](docs/JSON-REPORT.md);
- do not change the meaning/type of an existing schema-v1 field casually;
- validate representative clean, warning, error-diagnostic, and tool-failure reports against the committed schema.

## Documentation sources of truth

Durable project documentation lives in README, root project docs, and `docs/*.md`.

`docs/superpowers/` is reserved for local development-agent scratch specifications/plans and is intentionally ignored by Git. Do not link to it from Issues, PR descriptions, or durable project documentation.

## Rust version and quality gates

The workspace targets Rust **1.85.0** and Rust 2024 edition.

Run the full set relevant to your change:

```bash
cargo +stable fmt --all -- --check
cargo +stable clippy --workspace --all-targets --all-features -- -D warnings
cargo +1.85.0 check --workspace --all-targets --all-features
cargo +stable build --workspace --all-targets --all-features
cargo +stable test --workspace --all-features
```

Database-backed integration tests require `PGPREFLIGHT_TEST_DATABASE_URL`. CI runs the complete workspace suite separately against PostgreSQL 14, 15, 16, 17, and 18. The non-database build/test matrix runs on Linux, macOS, and Windows without that environment variable.

The stable branch-protection target is `CI / required`, which aggregates quality, MSRV, cross-platform, PostgreSQL, and release-readiness jobs.

## First crates.io release

Cargo resolves registry dependencies during both `cargo publish --dry-run` and `cargo package`, even with `--no-verify`. Before the first release, CI therefore runs a full dry-run for `pgpreflight-core` and prepares the two dependent packages with temporary local registry patches. The generated manifests still contain their crates.io version dependencies. Publish in dependency order and repeat the dry-run immediately before each upload:

```bash
cargo publish --dry-run --locked -p pgpreflight-core
cargo publish --locked -p pgpreflight-core
cargo publish --dry-run --locked -p pgpreflight-postgres
cargo publish --locked -p pgpreflight-postgres
cargo publish --dry-run --locked -p pgpreflight
cargo publish --locked -p pgpreflight
```

Wait until crates.io exposes each dependency before checking or publishing the next crate.

## Pull requests

A good PR description should state:

- scope and explicit non-goals;
- tests/fixtures added or changed;
- safety implications;
- public API/schema/documentation impact;
- verification commands executed.

Avoid mixing unrelated refactors with a safety- or semantics-sensitive change.

## Contribution licensing

pgpreflight is licensed under **MIT OR Apache-2.0**. Unless explicitly stated otherwise, contributions intentionally submitted for inclusion are provided under the same dual-license terms.

## Security issues

Do not open a public issue containing sensitive vulnerability details. Follow [SECURITY.md](SECURITY.md).
