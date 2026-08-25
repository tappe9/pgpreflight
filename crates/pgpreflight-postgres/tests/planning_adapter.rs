use pgpreflight_core::PostgresConfig;
use pgpreflight_postgres::{PlanningError, SafeModePlanner, parse_and_validate};
use tokio_postgres::{Client, NoTls};

fn test_database_url() -> Option<String> {
    std::env::var("PGPREFLIGHT_TEST_DATABASE_URL").ok()
}

async fn connect_test_client(database_url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("test database must accept connections");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

#[tokio::test(flavor = "current_thread")]
async fn plans_update_without_executing_it_and_rolls_back_success() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let admin = connect_test_client(&database_url).await;
    admin
        .batch_execute(
            "DROP TABLE IF EXISTS pgpreflight_update_probe;\
             CREATE TABLE pgpreflight_update_probe (id integer PRIMARY KEY, value text NOT NULL);\
             INSERT INTO pgpreflight_update_probe VALUES (1, 'before');",
        )
        .await
        .unwrap();

    let mut planner = SafeModePlanner::connect(&database_url).await.unwrap();
    let statement =
        parse_and_validate("UPDATE pgpreflight_update_probe SET value = 'after' WHERE id = 1")
            .unwrap();
    let config = PostgresConfig::default();

    planner.plan(&statement, &config).await.unwrap();
    planner.plan(&statement, &config).await.unwrap();

    let row = admin
        .query_one(
            "SELECT value FROM pgpreflight_update_probe WHERE id = 1",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "before");

    admin
        .batch_execute("DROP TABLE pgpreflight_update_probe")
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn safe_mode_settings_are_active_during_planning() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let admin = connect_test_client(&database_url).await;
    admin
        .batch_execute(
            r#"
            CREATE OR REPLACE FUNCTION pgpreflight_safe_mode_probe()
            RETURNS integer
            LANGUAGE plpgsql
            IMMUTABLE
            AS $function$
            BEGIN
                IF current_setting('transaction_read_only') <> 'on' THEN
                    RAISE EXCEPTION 'read-only probe failed';
                END IF;
                IF current_setting('statement_timeout') <> '75ms' THEN
                    RAISE EXCEPTION 'statement-timeout probe failed';
                END IF;
                IF current_setting('lock_timeout') <> '25ms' THEN
                    RAISE EXCEPTION 'lock-timeout probe failed';
                END IF;
                RETURN 1;
            END;
            $function$;
            "#,
        )
        .await
        .unwrap();

    let ordinary_plan = admin
        .query(
            "EXPLAIN (FORMAT JSON, VERBOSE TRUE) SELECT pgpreflight_safe_mode_probe()",
            &[],
        )
        .await;
    assert!(ordinary_plan.is_err(), "probe must execute while planning");

    let mut planner = SafeModePlanner::connect(&database_url).await.unwrap();
    let statement = parse_and_validate("SELECT pgpreflight_safe_mode_probe()").unwrap();
    let config = PostgresConfig {
        statement_timeout_ms: 75,
        lock_timeout_ms: 25,
    };

    planner.plan(&statement, &config).await.unwrap();

    admin
        .batch_execute("DROP FUNCTION pgpreflight_safe_mode_probe()")
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn maps_lock_timeout_and_rolls_back_recoverable_failure_without_sql_leak() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let blocker = connect_test_client(&database_url).await;
    blocker
        .batch_execute(
            "DROP TABLE IF EXISTS pgpreflight_timeout_probe;\
             CREATE TABLE pgpreflight_timeout_probe (id integer PRIMARY KEY, value text NOT NULL);\
             INSERT INTO pgpreflight_timeout_probe VALUES (1, 'before');",
        )
        .await
        .unwrap();
    blocker
        .batch_execute(
            "BEGIN;\
             LOCK TABLE pgpreflight_timeout_probe IN ACCESS EXCLUSIVE MODE;",
        )
        .await
        .unwrap();

    let mut planner = SafeModePlanner::connect(&database_url).await.unwrap();
    let statement = parse_and_validate(
        "UPDATE pgpreflight_timeout_probe SET value = 'sql-secret-marker' WHERE id = 1",
    )
    .unwrap();
    let config = PostgresConfig {
        statement_timeout_ms: 500,
        lock_timeout_ms: 50,
    };

    let error = planner.plan(&statement, &config).await.unwrap_err();
    assert_eq!(error, PlanningError::Timeout);
    assert!(!error.to_string().contains("sql-secret-marker"));
    assert!(!format!("{error:?}").contains("sql-secret-marker"));

    blocker.batch_execute("ROLLBACK").await.unwrap();

    let follow_up = parse_and_validate("SELECT * FROM pgpreflight_timeout_probe").unwrap();
    planner
        .plan(&follow_up, &PostgresConfig::default())
        .await
        .unwrap();

    blocker
        .batch_execute("DROP TABLE pgpreflight_timeout_probe")
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn connection_failures_do_not_leak_credential_bearing_url() {
    let database_url = "postgresql://postgres:credential-secret-marker@127.0.0.1:1/postgres";

    let error = match SafeModePlanner::connect(database_url).await {
        Ok(_) => panic!("connection unexpectedly succeeded"),
        Err(error) => error,
    };

    assert_eq!(error, PlanningError::Connection);
    assert!(!error.to_string().contains("credential-secret-marker"));
    assert!(!error.to_string().contains(database_url));
    assert!(!format!("{error:?}").contains("credential-secret-marker"));
}
