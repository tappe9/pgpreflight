use pgpreflight_postgres::{CheckError, PlanningError, SafeModePlanner, parse_and_validate};

fn assert_absent(rendered: &str, forbidden: &[&str]) {
    for value in forbidden {
        assert!(
            !rendered.contains(value),
            "public error formatting leaked {value:?}: {rendered}"
        );
    }
}

#[test]
fn parser_failures_have_fixed_public_formatting_without_raw_parser_details() {
    let error = parse_and_validate("SELECT 'literal-secret-marker' FROM")
        .expect_err("invalid SQL should fail parsing");

    assert_eq!(error, CheckError::SqlParse);
    assert_eq!(error.to_string(), "SQL could not be parsed");
    let debug = format!("{error:?}");
    assert_absent(
        &format!("{}\n{debug}", error),
        &[
            "literal-secret-marker",
            "ParserError",
            "Expected",
            "sql parser error",
        ],
    );
}

#[tokio::test(flavor = "current_thread")]
async fn driver_failures_have_fixed_public_formatting_without_credentials_or_driver_details() {
    let database_url =
        "postgresql://user:credential-secret-marker@127.0.0.1:1/pgpreflight";
    let error = match SafeModePlanner::connect(database_url).await {
        Ok(_) => panic!("connection unexpectedly succeeded"),
        Err(error) => error,
    };

    assert_eq!(error, PlanningError::Connection);
    assert_eq!(error.to_string(), "database connection failed");
    let debug = format!("{error:?}");
    assert_absent(
        &format!("{}\n{debug}", error),
        &[
            "credential-secret-marker",
            database_url,
            "Connection refused",
            "os error",
            "tcp connect error",
        ],
    );
}
