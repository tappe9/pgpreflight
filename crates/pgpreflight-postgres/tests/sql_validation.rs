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
fn rejects_unsupported_outer_statements() {
    for sql in [
        "INSERT INTO orders(id) VALUES (1)",
        "MERGE INTO orders USING incoming ON orders.id = incoming.id WHEN MATCHED THEN DELETE",
        "COPY orders TO STDOUT",
        "CALL refresh_orders()",
        "CREATE TABLE x(id int)",
        "BEGIN",
        "EXPLAIN SELECT * FROM orders",
    ] {
        let error = parse_and_validate(sql).unwrap_err();
        assert!(matches!(
            error,
            CheckError::UnsupportedStatement | CheckError::UnsafeConstruct { .. }
        ));
    }
}

#[test]
fn parse_errors_are_sanitized() {
    let error = parse_and_validate("SELECT 'super-secret").unwrap_err();
    assert!(matches!(error, CheckError::SqlParse));
    assert!(!error.to_string().contains("super-secret"));
}
