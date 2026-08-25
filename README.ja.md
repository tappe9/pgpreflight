# pgpreflight

[![CI](https://github.com/tappe9/pgpreflight/actions/workflows/ci.yml/badge.svg)](https://github.com/tappe9/pgpreflight/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#ライセンス)

`pgpreflight` は、アプリケーションが実行する前に、1つの PostgreSQL `SELECT` / `UPDATE` / `DELETE` を PostgreSQL 本体の planner に照らして事前検査するための Rust CLI / library プロジェクトです。

v0.1 では、保守的な SQL validation、plain `EXPLAIN (FORMAT JSON, VERBOSE TRUE)`、catalog statistics、決定論的 diagnostics、versioned JSON report を組み合わせる設計です。SQLを実行するツールでも、`EXPLAIN ANALYZE` のwrapperでもありません。

> **開発状況:** pre-releaseで開発中です。現在の `main` には workspace基盤、core configuration/report contract、JSON Schema v1、保守的なSQL parse/validationまで実装済みです。PostgreSQL Safe Mode planning、plan normalization、diagnostic評価、利用者向け `check` CLI は今後のv0.1実装です。

English: [README.md](README.md)

## pgpreflight が解決したいこと

SQLが構文的に正しくても、安全・妥当とは限りません。`UPDATE` の `WHERE` 抜け、広範囲の sequential scan、想定外に大きい result set などは、実行前に気付きたい問題です。一方、静的なSQL解析だけでは PostgreSQL の name resolution、permission、statistics、planner choice、server version差を再現できません。

そのため pgpreflight は責務を分けます。

1. PostgreSQL dialect の AST validator が「このSQLをpgpreflightが扱ってよいか」を保守的に判断する。
2. validationを通過したSQLについては PostgreSQL 自身を semantic / planning authority とする。

安全性を確立できない構文は、PostgreSQLへ送る前にfail closedします。

## 現在 `main` に実装済みの範囲

- `pgpreflight-core` / `pgpreflight-postgres` / `pgpreflight` の Rust 2024 Cargo workspace
- strictかつversionedなcore configurationとv0.1 default threshold
- PostgreSQL driver / parser ASTをcore public APIへ漏らさないnormalized model、diagnostic、report型
- `schemas/report-v1.schema.json`
- `sqlparser-rs` PostgreSQL dialectによるparse
- exactly-one-statement validation
- `SELECT` / `UPDATE` / `DELETE` の保守的accept
- direct `EXPLAIN`、locking query、`SELECT INTO`、data-modifying nested query、unsupported statementのreject
- accepted / rejected / known-unsupported SQL corpus
- Rust 1.85.0によるLinux quality CIとmacOS/Windows cross-platform check

まだend-to-endで未実装の範囲:

- PostgreSQL connectionとSafe Mode transaction
- `EXPLAIN` / catalog read
- plan/statistics normalization
- PGP001〜PGP104 rule evaluation
- `pgpreflight check <INPUT>` のconfig discovery、rendering、exit code
- PostgreSQL 14〜18 integration matrixとrelease packaging

実装順は [ROADMAP.md](ROADMAP.md) を参照してください。

## v0.1 の処理フロー

```text
SQL file / stdin
      │
      ▼
UTF-8 + single-statement validation
      │
      ▼
conservative PostgreSQL AST safety gate
      │
      ▼
PostgreSQL connection
      │
      ▼
read-only transaction + local timeouts
      │
      ▼
EXPLAIN (FORMAT JSON, VERBOSE TRUE)
      │
      ├── catalog statistics
      ▼
normalized statement / plan / relation facts
      │
      ▼
deterministic PGP001–PGP104 rules
      │
      ├── text report
      └── JSON report schema v1
```

planning pathでは plain `EXPLAIN` のみを使用し、**`EXPLAIN ANALYZE` は使用しません**。

## SQL policy

v0.1は意図的に対象を限定します。

- exactly one statement
- outer statementは `SELECT` / `UPDATE` / `DELETE` のみ
- direct `EXPLAIN` はreject
- locking clauseはreject
- `SELECT INTO` はreject
- data-modifying nested query / CTEはreject
- unsupported / ambiguous formはfail closed
- parameter placeholderはv0.1対象外で、literal valueを含む1 statementを想定

parserはPostgreSQLのsemantic authorityを置き換えるものではありません。詳細は [SQL support](docs/SQL-SUPPORT.md) を参照してください。

## v0.1 diagnostics

| Rule | Severity | 内容 |
| --- | --- | --- |
| `PGP001` | error | `WHERE` のない `UPDATE` |
| `PGP002` | error | `WHERE` のない `DELETE` |
| `PGP101` | warning | 影響行数の推定が大きい |
| `PGP102` | warning | 大きなrelationへの低selectivity sequential scan |
| `PGP103` | warning | `SELECT` result set推定が大きい |
| `PGP104` | warning | 保守的に証明できるCartesian join risk |

configuration typeとdefault値はすでに `pgpreflight-core` に存在しますが、rule engine本体は未実装です。詳細は [Rules](docs/RULES.md) を参照してください。

## Safety model

v0.1では複数の独立したguardを重ねます。

- DB access前のconservative AST validation
- read-only transaction
- local statement / lock timeout
- plain `EXPLAIN` only
- target DMLを意図的に実行しない
- SQL textやcredentialを保持しないsanitized error/report model
- least-privilege connectionの推奨

ただし、これは **PostgreSQLの万能sandboxではありません**。planner hook、FDW、extension、誤ってimmutable等と宣言されたuser-defined functionは、planning中にpgpreflight管理外の動作を行う可能性があります。

[Safety model](docs/SAFETY.md) と [Security Policy](SECURITY.md) を確認してください。

## Workspace

```text
pgpreflight/
├── crates/
│   ├── pgpreflight-core/      # normalized model / config / diagnostics / report
│   ├── pgpreflight-postgres/  # PostgreSQL parser / safety / planning adapter
│   └── pgpreflight/           # CLI
├── docs/
├── schemas/
└── tests/
```

依存方向:

```text
pgpreflight -> pgpreflight-postgres -> pgpreflight-core
pgpreflight -----------------------> pgpreflight-core
```

## 現在利用できるlibrary surface

`pgpreflight-postgres` は現在、保守的なvalidation entry pointを公開しています。

```rust
use pgpreflight_postgres::parse_and_validate;

let validated = parse_and_validate("UPDATE public.accounts SET active = false WHERE id = 42")?;
println!("{:?}", validated.facts().kind);
# Ok::<(), pgpreflight_postgres::CheckError>(())
```

`ValidatedStatement` は `sqlparser-rs` ASTをpublic APIへ公開しません。接続済みplanning facadeはv0.1 targetで、現時点では未実装です。

## Compatibility target

- Rust: **1.85.0+** (`edition = 2024`)
- PostgreSQL: **14〜18 target**。database integration coverageは未完了
- OS target: Linux x86_64 / macOS aarch64・x86_64 / Windows x86_64

現在検証済みの範囲との違いは [Compatibility](docs/COMPATIBILITY.md) に記載します。

## Documentation

- [Requirements](docs/REQUIREMENTS.md)
- [Architecture](ARCHITECTURE.md)
- [Public API design](docs/API-DESIGN.md)
- [SQL support / validation policy](docs/SQL-SUPPORT.md)
- [Safety model](docs/SAFETY.md)
- [Diagnostic rules](docs/RULES.md)
- [JSON report contract](docs/JSON-REPORT.md)
- [Compatibility](docs/COMPATIBILITY.md)
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Security Policy](SECURITY.md)

`docs/superpowers/` 以下のdevelopment-agent用scratch planは正式なproject documentationではないため、Gitの追跡対象外です。

## v0.1 Non-goals

- SQL execution / `EXPLAIN ANALYZE`
- exact runtime prediction
- SQL rewrite / automatic fix / index creation
- hypothetical index
- DDL / migration analysis
- `INSERT` / `MERGE` / `COPY` / `CALL` / `DO`
- parameter binding / prepared-statement emulation
- batch / glob / directory processing
- telemetry / crash upload / query history
- arbitrary PostgreSQL extension/functionに対する万能sandbox

## Contributing

小さくtest可能な実装単位で開発します。詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。

## ライセンス

以下のいずれかを選択できます。

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
