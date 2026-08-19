use std::{fs, path::PathBuf};

use pgpreflight_core::StatementKind;
use pgpreflight_postgres::{CheckError, parse_and_validate};

#[test]
fn accepts_single_select() {
    let validated = parse_and_validate("SELECT * FROM orders WHERE id = 1").unwrap();
    assert_eq!(validated.facts().kind, StatementKind::Select);
}

#[test]
fn accepts_update_and_delete() {
    let update = parse_and_validate("UPDATE orders SET status = 'done' WHERE id = 1").unwrap();
    let delete = parse_and_validate("DELETE FROM orders WHERE id = 1").unwrap();

    assert_eq!(update.facts().kind, StatementKind::Update);
    assert!(update.facts().has_where);
    assert_eq!(delete.facts().kind, StatementKind::Delete);
    assert!(delete.facts().has_where);
}

#[test]
fn rejects_multiple_statements() {
    let error = parse_and_validate("SELECT 1; SELECT 2;").unwrap_err();
    assert!(matches!(error, CheckError::MultipleStatements));
}

#[test]
fn rejects_locking_select() {
    let error = parse_and_validate("SELECT * FROM orders FOR UPDATE").unwrap_err();
    assert!(matches!(error, CheckError::UnsafeConstruct { .. }));
}

#[test]
fn rejects_nested_locking_select() {
    let sql = "SELECT * FROM (SELECT * FROM orders FOR UPDATE) AS locked_orders";
    let error = parse_and_validate(sql).unwrap_err();
    assert!(matches!(error, CheckError::UnsafeConstruct { .. }));
}

#[test]
fn rejects_data_modifying_delete_cte() {
    let sql =
        "WITH removed AS (DELETE FROM orders WHERE id = 1 RETURNING id) SELECT * FROM removed";
    let error = parse_and_validate(sql).unwrap_err();
    assert!(matches!(error, CheckError::UnsafeConstruct { .. }));
}

#[test]
fn rejects_nested_data_modifying_delete_cte() {
    let sql = concat!(
        "SELECT * FROM (WITH removed AS (",
        "DELETE FROM orders WHERE id = 1 RETURNING id",
        ") SELECT * FROM removed) AS nested"
    );
    let error = parse_and_validate(sql).unwrap_err();
    assert!(matches!(error, CheckError::UnsafeConstruct { .. }));
}

#[test]
fn rejects_unsupported_outer_statements() {
    for sql in [
        "INSERT INTO orders(id) VALUES (1)",
        "MERGE INTO orders USING incoming ON orders.id = incoming.id WHEN MATCHED THEN DELETE",
        "COPY orders TO STDOUT",
        "CALL refresh_orders()",
        "DO $$ BEGIN NULL; END $$",
        "CREATE TABLE x(id int)",
        "BEGIN",
        "EXPLAIN SELECT * FROM orders",
    ] {
        let error = parse_and_validate(sql).unwrap_err();
        assert!(matches!(
            error,
            CheckError::SqlParse
                | CheckError::UnsupportedStatement
                | CheckError::UnsafeConstruct { .. }
        ));
    }
}

#[test]
fn parse_errors_are_sanitized() {
    let error = parse_and_validate("SELECT 'super-secret").unwrap_err();
    assert!(matches!(error, CheckError::SqlParse));
    assert!(!error.to_string().contains("super-secret"));
}

#[test]
fn accepted_corpus_is_accepted() {
    for path in corpus_files("accepted") {
        let sql = fs::read_to_string(&path).unwrap();
        parse_and_validate(&sql)
            .unwrap_or_else(|error| panic!("{} was rejected: {error}", path.display()));
    }
}

#[test]
fn rejected_corpus_has_expected_stable_category() {
    for path in corpus_files("rejected") {
        let sql = fs::read_to_string(&path).unwrap();
        let error = parse_and_validate(&sql).unwrap_err();
        let name = path.file_name().unwrap().to_string_lossy();

        if name.starts_with("unsafe__") {
            assert!(
                matches!(error, CheckError::UnsafeConstruct { .. }),
                "{} returned {error:?}",
                path.display()
            );
        } else if name.starts_with("unsupported__") {
            assert!(
                matches!(error, CheckError::UnsupportedStatement),
                "{} returned {error:?}",
                path.display()
            );
        } else {
            panic!("rejected corpus file needs a stable-category prefix: {name}");
        }
    }
}

fn corpus_files(category: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/sql-corpus")
        .join(category);
    let mut files = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect::<Vec<_>>();
    files.sort();
    files
}
