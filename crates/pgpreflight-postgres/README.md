# pgpreflight-postgres

PostgreSQL-specific validation, Safe Mode planning, plan normalization, and relation-statistics adapters for [`pgpreflight`](https://github.com/tappe9/pgpreflight).

Planning uses plain `EXPLAIN`, never `EXPLAIN ANALYZE`.
