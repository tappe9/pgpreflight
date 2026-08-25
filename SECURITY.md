# Security Policy

pgpreflight processes SQL text and is designed to connect to PostgreSQL, so both **query confidentiality** and **database-side planning behavior** are security-sensitive concerns.

## Supported versions

pgpreflight is currently pre-release. Until v0.1.0 is published, security fixes are made on the latest `main` development line only.

After pre-1.0 releases begin, the project intends to support only the latest published pre-1.0 release unless a release note states otherwise.

## Reporting a vulnerability

Do not publish exploit details, credentials, production SQL, or other sensitive reproduction material in a public issue.

Preferred process:

1. use GitHub private vulnerability reporting / Security Advisories for this repository when available;
2. if private reporting is unavailable, open a minimal public issue asking the maintainer for a private contact path without including vulnerability details.

A useful private report includes:

- affected commit/version;
- whether the issue affects parsing, Safe Mode, redaction, plan handling, configuration, or CLI behavior;
- impact and prerequisites;
- a minimized reproduction that contains no real credentials or sensitive production data;
- whether PostgreSQL extensions, FDWs, hooks, or user-defined functions are involved.

## Security properties

Security-relevant invariants include:

- unsupported SQL fails closed before database planning;
- the planning adapter must never add `ANALYZE`;
- target DML must never be sent outside the plain `EXPLAIN` wrapper;
- planning occurs in a read-only transaction with local timeouts;
- passwords and complete credential-bearing URLs are not emitted by public errors or normal output;
- SQL text and literal values are not retained in stable normalized report models;
- raw verbose plans are treated as sensitive transient data;
- malformed/unsupported SQL returns typed sanitized errors rather than panics.

The parser/validation portion of this model is implemented today. Database Safe Mode and end-to-end redaction behavior remain v0.1 work and must not be assumed complete before their corresponding issues land.

## Known trust boundary

Plain `EXPLAIN` is safer than `EXPLAIN ANALYZE` for DML because it plans rather than intentionally executes the target statement. It is **not a universal sandbox**.

PostgreSQL planner hooks, FDWs, extensions, and user-defined functions with unusual or incorrectly declared behavior may perform work during planning. pgpreflight cannot guarantee zero side effects from arbitrary server-side code.

Use a dedicated least-privilege role and prefer local, development, staging, or sanitized replica databases. Treat production connectivity as a deliberate security decision.

See [docs/SAFETY.md](docs/SAFETY.md) for the detailed operational safety model.

## Secret-handling expectations

Security reports, tests, fixtures, and examples must not commit live connection URLs, passwords, tokens, or sensitive SQL literals. Regression tests should use conspicuous synthetic secret markers and assert that they never reach user-visible output.
