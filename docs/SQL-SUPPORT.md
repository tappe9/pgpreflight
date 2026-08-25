# SQL Support and Validation Policy

Status: **current parser/validator behavior plus v0.1 policy**

pgpreflight does not send every statement that `sqlparser-rs` can parse to PostgreSQL. Parsing and admission are separate steps.

## 1. Core rule

The validator must be able to establish that the entire relevant statement/query shape fits the supported v0.1 safety policy. Otherwise it rejects the input.

This is intentionally conservative: false negatives (rejecting a statement pgpreflight could theoretically inspect) are preferable to accidentally admitting a modifying/locking form outside the reviewed policy.

## 2. Statement count

`parse_and_validate()` requires exactly one parsed SQL statement.

- one statement: continue to policy validation;
- zero statements: parse/validation failure;
- more than one statement: `CheckError::MultipleStatements`.

The future CLI additionally defines UTF-8, BOM, empty/comment-only input behavior before/around this parser boundary.

## 3. Supported outer statements

The current validator admits these outer families when their nested query structure is also safe:

- `SELECT`;
- `UPDATE`;
- `DELETE`.

All other outer statement types are unsupported for v0.1. This includes, among others, `INSERT`, `MERGE`, DDL, transaction control, `COPY`, `CALL`, and `DO`.

Direct `EXPLAIN` is rejected explicitly as an unsafe construct because pgpreflight owns the `EXPLAIN` wrapper and its options.

## 4. Recursively rejected query constructs

The validator traverses nested queries and rejects:

### Locking clauses

Any query with PostgreSQL locking semantics represented in the AST (for example `FOR UPDATE` / `FOR SHARE` families) is rejected as an unsafe locking clause.

### Data-modifying query bodies

Nested query bodies containing `INSERT`, `UPDATE`, `DELETE`, or `MERGE` are rejected. This prevents data-modifying CTE/query forms from being hidden under an otherwise supported outer query.

### `SELECT INTO`

`SELECT ... INTO` is rejected recursively because it creates a relation rather than behaving as a read-only result query.

## 5. SELECT query bodies

Current `SELECT` admission accepts select-shaped query bodies and recursively select-shaped set operations. Non-select set-expression forms are rejected as unsupported.

Admission means only that the local AST safety policy passed. PostgreSQL may still reject the statement later for syntax-version, name-resolution, type, permission, or semantic reasons during connected planning.

## 6. Statement facts and join ownership

Validation produces normalized `StatementFacts` used by later analysis.

Current facts include:

- `StatementKind::{Select, Update, Delete}`;
- target relation when it can be represented by the current normalized relation form;
- whether a syntactic `WHERE` exists;
- whether `RETURNING` exists;
- a conservative relation-occurrence `JoinGraph` for PGP104.

A syntactic `WHERE TRUE` still counts as a present `WHERE`; rule PGP001/PGP002 intentionally check syntax presence, not predicate selectivity.

The join graph is intentionally narrower than SQL admission. It records only ownership that can be proven without PostgreSQL name-resolution guesses:

- direct base-table occurrences are vertices;
- aliases identify occurrences independently, including self joins;
- qualified cross-relation `WHERE`/`ON` predicates, `USING`, and `NATURAL JOIN` may add edges;
- `CROSS JOIN` and `ON TRUE` add no edge by themselves;
- later supported predicates can connect earlier relation groups;
- unqualified/ambiguous ownership, duplicate qualifiers, derived or `LATERAL` relations, correlated subqueries, CTE/set-operation ownership, and unsupported join shapes mark the graph `indeterminate`.

An indeterminate graph does not reject an otherwise admitted statement. It only causes PGP104 to skip rather than emit a speculative warning. This distinction preserves PostgreSQL as semantic authority while keeping the diagnostic conservative.

The graph builder covers direct select-shaped `SELECT`, `UPDATE ... FROM`, and `DELETE ... USING` forms admitted by the current policy. It stores safe relation identity/alias and normalized edges only; SQL text, literals, and raw expressions are not retained.

## 7. Known unsupported corpus

Repository fixtures are split into:

```text
tests/sql-corpus/
├── accepted/
├── rejected/
└── known-unsupported/
```

- **accepted**: statements that define supported behavior;
- **rejected**: statements that must fail the current policy;
- **known-unsupported**: useful PostgreSQL forms intentionally outside the current supported subset.

Moving a case from known-unsupported to accepted requires a reviewed safety rationale and focused tests, not only a parser-library upgrade.

## 8. Parser vs PostgreSQL authority

`sqlparser-rs` is not used as a PostgreSQL compatibility oracle. Its job is to provide enough structure for the pre-planning safety gate and conservative normalized facts.

During connected planning, PostgreSQL remains authoritative for:

- name resolution;
- type checking;
- permissions;
- server-version syntax;
- functions/operators;
- planner behavior;
- catalog/statistics semantics.

If PostgreSQL supports syntax the local parser cannot safely represent, pgpreflight rejects it rather than bypassing the gate.

If a statement is admitted but its relation ownership cannot be established locally, PostgreSQL may still plan it successfully while PGP104 skips.

## 9. Parameters

The v0.1 workflow is defined for one statement containing literal values. `$1`-style parameter binding and prepared-statement emulation are outside v0.1 because meaningful planning may depend on values and PostgreSQL parameter semantics.
