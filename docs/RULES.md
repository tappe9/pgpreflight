# Diagnostic Rules

Status: **v0.1 rule contract; PGP001/PGP002/PGP101 implemented, PGP102–PGP104 planned**

pgpreflight v0.1 defines six deterministic rules. Severity is fixed in v0.1; users can enable/disable rules and configure approved numeric thresholds.

## Rule summary

| Rule | Severity | Statements | Meaning |
| --- | --- | --- | --- |
| `PGP001` | error | `UPDATE` | missing syntactic `WHERE` |
| `PGP002` | error | `DELETE` | missing syntactic `WHERE` |
| `PGP101` | warning | `UPDATE`, `DELETE` | large estimated affected row set |
| `PGP102` | warning | supported statements | large relation sequential scan with low output ratio |
| `PGP103` | warning | `SELECT` | large estimated result set |
| `PGP104` | warning | supported join-bearing queries | conservatively provable disconnected join graph |

All six rules default to enabled in the current core configuration model.

## PGP001 — UPDATE without WHERE

Trigger when a validated `UPDATE` has no syntactic `WHERE` clause.

- severity: `error`;
- does not require planner evidence;
- `WHERE TRUE` counts as a present `WHERE` in v0.1.

The rule is intentionally syntactic. Predicate quality/selectivity belongs to other evidence.

## PGP002 — DELETE without WHERE

Same contract as PGP001 for `DELETE`.

- severity: `error`;
- no affected-row estimate required;
- `WHERE TRUE` counts as present.

## PGP101 — Large affected row set

Default configuration:

```toml
[rules.PGP101]
enabled = true
max_rows = 10000
max_table_ratio = 0.05
min_rows_for_ratio = 1000
```

Warn when either:

```text
estimated_affected_rows >= max_rows
```

or:

```text
estimated_affected_rows >= min_rows_for_ratio
AND relation_rows is known and positive
AND estimated_affected_rows / relation_rows >= max_table_ratio
```

If relation statistics are unavailable, only the absolute threshold is evaluated. The implementation does not invent relation size.

## PGP102 — Large sequential scan

Default configuration:

```toml
[rules.PGP102]
enabled = true
min_relation_rows = 100000
max_output_ratio = 0.20
```

For each normalized sequential-scan node, warn when:

```text
relation_rows >= min_relation_rows
AND scan_output_rows / relation_rows <= max_output_ratio
```

`Plan Rows` represents estimated output rows, not physical rows read. Catalog relation rows approximate the table size needed for this heuristic.

Skip when relation statistics are missing/non-positive. Self-join scan nodes are evaluated independently.

## PGP103 — Large estimated result set

Default configuration:

```toml
[rules.PGP103]
enabled = true
max_result_rows = 100000
```

For `SELECT`, warn when the normalized root estimated rows meet/exceed the threshold. Using the root means upper plan nodes such as `LIMIT` and aggregation naturally affect the estimate.

`UPDATE ... RETURNING` and `DELETE ... RETURNING` are outside this rule in v0.1.

## PGP104 — Possible Cartesian join

PGP104 uses a conservative relation-occurrence graph rather than string-searching SQL.

Design rules:

- each relevant relation occurrence is a vertex;
- provable cross-relation predicates may add edges;
- `USING` and `NATURAL JOIN` connect operands when ownership is clear;
- `CROSS JOIN` or `JOIN ... ON TRUE` does not itself add predicate evidence;
- a later supported predicate may connect previously separate components;
- aliases distinguish repeated/self-join relation occurrences;
- ambiguous ownership, unsupported `LATERAL`, complex correlated cases, and unsupported set-returning behavior cause conservative skip rather than a guessed warning.

Warn when at least two relevant relation occurrences remain in multiple provable connected components.

## Evidence and ordering

Diagnostics carry typed evidence variants. Consumers should not parse human-readable messages to recover numeric values or relation identities.

v0.1 ordering is deterministic:

1. `error` before `warning`;
2. rule ID;
3. relation identity when relevant;
4. stable plan traversal order when relevant.

## Configuration validation

Current core config validation rejects:

- unsupported config version;
- PGP101/PGP102 ratios outside `0.0..=1.0`;
- negative row thresholds.

Rule severity is not configurable in v0.1.
