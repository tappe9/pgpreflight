# JSON Report Contract

Status: **schema v1 and typed report model implemented; CLI emission planned**

pgpreflight uses an explicit report schema version so automation can depend on a documented machine boundary independently of the pre-1.0 Rust API.

Canonical machine-readable schema:

- [`schemas/report-v1.schema.json`](../schemas/report-v1.schema.json)

## 1. Top-level shape

Schema v1 requires:

```json
{
  "schema_version": 1,
  "tool": {
    "name": "pgpreflight",
    "version": "0.1.0"
  },
  "status": "clean",
  "statement": {
    "kind": "select"
  },
  "summary": {
    "errors": 0,
    "warnings": 0
  },
  "diagnostics": [],
  "failure": null
}
```

`statement` may be `null` for failures that occur before a supported statement can be identified.

## 2. Status values

Schema v1 defines:

- `clean` — analysis completed with no diagnostics;
- `warnings` — analysis completed with warning diagnostics and no errors;
- `errors` — analysis completed with one or more error diagnostics;
- `failed` — pgpreflight could not complete analysis.

A diagnostic result and a tool failure are deliberately different concepts.

## 3. Diagnostics

Each diagnostic includes at least:

- `rule_id`;
- `severity`;
- `title`;
- `message`;
- structured `evidence`.

Optional structured threshold data may be included. Machine consumers should branch on `rule_id`, `severity`, and evidence fields rather than matching message text.

Schema v1 recognizes rule IDs `PGP001`, `PGP002`, `PGP101`, `PGP102`, `PGP103`, and `PGP104`.

## 4. Failure

For tool failures, `failure` contains a stable non-empty `kind` and sanitized human message. The design intentionally avoids placing raw SQL, passwords, complete connection URLs, or raw verbose plan data in this object.

The exact adapter/CLI failure taxonomy will be completed with their implementation slices.

## 5. CLI JSON-stream contract

The planned `--format json` behavior is:

- stdout contains exactly one JSON object for clean, diagnostic, and structured tool-failure outcomes;
- stderr remains empty for those structured outcomes;
- no ANSI escapes are emitted;
- process exit status still follows the CLI failure/`--fail-on` policy.

This behavior is not yet available because the end-user CLI is not implemented.

## 6. Schema-v1 compatibility policy

While `schema_version` remains `1`:

- existing required fields are not removed;
- existing field types/meanings are not changed incompatibly;
- new required top-level fields are not added casually;
- optional fields and new diagnostic/evidence variants may be added when compatible with the documented consumer policy;
- consumers should ignore unknown optional fields.

A change that requires consumers to reinterpret existing fields should use a new schema version.

## 7. Source of truth

The JSON Schema is the machine-readable source of truth for structural validation. This document explains semantics but should not duplicate every schema constraint.

Contract tests should validate representative clean, warning, error-diagnostic, and tool-failure objects against the committed schema before v0.1 release.
