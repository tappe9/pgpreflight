# Diagnostic Rules

Status: **v0.1 rule contract; PGP001–PGP104 implemented**

pgpreflight v0.1 defines six deterministic rules. Severity is fixed in v0.1; users can enable/disable rules and configure approved numeric thresholds.

## Rule summary

| Rule | Severity | Statements | Meaning |
| --- | --- | --- | --- |
| `PGP001` | error | `UPDATE` | missing syntactic `WHERE` |
| `PGP002` | error | `DELETE` | missing syntactic `WHERE` |
| `PGP101` | warning | `UPDATE`, `DELETE` | large estimated affected row set |
| `PGP102` | warning | supported statements | large relation sequential scan with low output ratio |
| `PGP103` | warning | `SELECT` | large estimated result set |
| `PGP104` | warning | supported join-bearing `SELECT`, `UPDATE`, `DELETE` | conservatively provable disconnected join graph |

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

PGP104 uses a conservative relation-occurrence graph built at the validated-AST boundary rather than string-searching SQL.

Graph rules:

- each relevant base-relation occurrence is a vertex;
- qualified `WHERE`/`ON` atoms that provably reference multiple relation occurrences add edges;
- `USING` and `NATURAL JOIN` connect operands when ownership and operand connectivity are clear;
- `CROSS JOIN` and `JOIN ... ON TRUE` do not themselves add predicate evidence;
- later supported `WHERE` or `ON` predicates may connect previously separate components;
- aliases distinguish repeated/self-join relation occurrences;
- unqualified or ambiguous ownership, duplicate qualifiers, `LATERAL`/derived relations, correlated subqueries, unsupported join operators, and other shapes that cannot be proven set `indeterminate` and cause the rule to skip.

The implemented coverage includes direct select-shaped `SELECT`, validated `UPDATE ... FROM`, and validated `DELETE ... USING` statements. Set operations, CTE-backed ownership, and unsupported relation factors conservatively skip PGP104 rather than guessing.

Warn only when the graph is determinate, contains at least two relation occurrences, and has more than one connected component.

PGP104 evidence contains:

- deterministic disconnected relation-occurrence groups with safe schema/name/alias identity;
- normalized root estimated rows for `SELECT`, or normalized affected-row estimates for `UPDATE`/`DELETE` when available.

It does not retain complete SQL, literals, raw predicates, or raw plan expressions.

## Evidence and ordering

Diagnostics carry typed evidence variants. Consumers should not parse human-readable messages to recover numeric values or relation identities.

v0.1 ordering is deterministic:

1. `error` before `warning`;
2. rule ID;
3. relation identity when relevant;
4. stable plan traversal or relation-occurrence order when relevant.

## Configuration validation

Current core config validation rejects:

- unsupported config version;
- PGP101/PGP102 ratios outside `0.0..=1.0`;
- negative row thresholds.

Rule severity is not configurable in v0.1.
