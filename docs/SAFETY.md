# Safety Model

Status: **v0.1 contract; parser gate implemented, database Safe Mode planned**

pgpreflight is designed to reduce the chance of accidentally executing risky DML while obtaining PostgreSQL planner evidence. It does not claim that planning arbitrary PostgreSQL code is side-effect free.

## 1. Defense in depth

The v0.1 design uses independent layers:

1. conservative AST admission;
2. least-privilege database credentials;
3. read-only transaction;
4. transaction-local lock and statement timeouts;
5. plain `EXPLAIN` only;
6. no intentional target-DML execution;
7. normalization/redaction before stable reporting.

No single layer is described as a complete sandbox.

## 2. Pre-database safety gate

Implemented today, `parse_and_validate()`:

- requires one statement;
- accepts only the reviewed `SELECT` / `UPDATE` / `DELETE` family;
- rejects direct `EXPLAIN`;
- rejects locking clauses recursively;
- rejects data-modifying nested queries recursively;
- rejects `SELECT INTO` recursively;
- fails closed on unsupported statement/query forms.

See [SQL-SUPPORT.md](SQL-SUPPORT.md).

## 3. Planned Safe Mode transaction

The adapter must implement behavior equivalent to:

```sql
BEGIN READ ONLY;
SET LOCAL statement_timeout = '3000ms';
SET LOCAL lock_timeout = '500ms';
EXPLAIN (FORMAT JSON, VERBOSE TRUE) <validated statement>;
-- minimum required catalog reads
ROLLBACK;
```

The exact client protocol sequence may differ, but the invariants must not.

### Required invariants

- transaction state is read-only before planning;
- both timeouts are scoped with `SET LOCAL`;
- target SQL is sent only inside pgpreflight's plain `EXPLAIN` wrapper;
- `ANALYZE` is never enabled;
- success and recoverable errors explicitly roll back;
- disconnect is surfaced as failure while relying on PostgreSQL to clean up the abandoned transaction.

## 4. Why plain EXPLAIN is not a universal sandbox

Plain `EXPLAIN` normally plans DML rather than executing the target modification. PostgreSQL planning may nevertheless invoke server-side behavior outside pgpreflight's control, including:

- planner hooks installed by extensions;
- FDW callbacks;
- extension-specific behavior;
- user-defined functions whose declared volatility/safety does not reflect their real effects;
- other server-side code involved in parsing/planning.

Therefore pgpreflight must never claim "zero side effects for arbitrary PostgreSQL installations".

## 5. Operational guidance

Recommended environment:

- local development database;
- dedicated development/staging database;
- sanitized replica or purpose-built analysis database.

Recommended role:

- dedicated login;
- only privileges necessary to parse/plan the intended statements;
- no superuser or unnecessary role membership;
- no unrelated write/admin capabilities.

Planning `UPDATE`/`DELETE` may require corresponding table privileges even though the transaction itself is read-only.

Production connectivity should be deliberate and reviewed rather than the default usage pattern.

## 6. Timeouts

Current config defaults reserve:

```toml
statement_timeout_ms = 3000
lock_timeout_ms = 500
```

These are v0.1 defaults, not universal safe values. They bound planning/lock waits once the adapter is implemented; they do not bound every possible server-side resource or extension behavior.

## 7. Sensitive information

Treat these as sensitive transient data:

- input SQL and literal values;
- passwords;
- complete credential-bearing database URLs;
- raw PostgreSQL error strings when they can embed SQL;
- raw verbose plan JSON and arbitrary expression strings.

Stable public models should contain only normalized non-sensitive evidence required for diagnostics.

## 8. Error redaction

Public errors should classify failures without echoing sensitive source material. SQL parse errors currently expose a sanitized fixed message rather than parser-library details that may contain the source query.

The connected adapter and CLI must preserve this approach for connection, timeout, planning, catalog, and rendering failures.

## 9. Validation requirements for Safe Mode implementation

The adapter issue is not complete until integration tests prove at least:

- an `UPDATE` can be planned without changing the target row;
- `transaction_read_only` is `on` during planning;
- timeout categories are mapped without leaking test secret markers;
- no `EXPLAIN ANALYZE` construction exists in the production path;
- rollback behavior is exercised.
