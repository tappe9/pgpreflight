use pgpreflight_core::{Config, RuleId, analyze};
use pgpreflight_postgres::{SafeModePlanner, parse_and_validate};
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
async fn planned_join_graph_drives_pgp104_without_retaining_sql_literals() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let admin = connect_test_client(&database_url).await;
    admin
        .batch_execute(
            "DROP TABLE IF EXISTS pgpreflight_cartesian_left;\
             DROP TABLE IF EXISTS pgpreflight_cartesian_right;\
             CREATE TABLE pgpreflight_cartesian_left (id integer, marker text);\
             CREATE TABLE pgpreflight_cartesian_right (id integer);\
             INSERT INTO pgpreflight_cartesian_left VALUES (1, 'left'), (2, 'left');\
             INSERT INTO pgpreflight_cartesian_right VALUES (1), (2);\
             ANALYZE pgpreflight_cartesian_left;\
             ANALYZE pgpreflight_cartesian_right;",
        )
        .await
        .unwrap();

    let mut planner = SafeModePlanner::connect(&database_url).await.unwrap();
    let mut config = Config::default();
    config.rules.pgp102.enabled = false;
    config.rules.pgp103.enabled = false;

    let disconnected = parse_and_validate(
        "SELECT 'pgpreflight-secret-literal' AS marker \
         FROM public.pgpreflight_cartesian_left AS l \
         CROSS JOIN public.pgpreflight_cartesian_right AS r",
    )
    .unwrap();
    let disconnected_plan = planner.plan(&disconnected, &config.postgres).await.unwrap();
    let disconnected_report = analyze(disconnected_plan.analysis_input(), &config);

    let diagnostic = disconnected_report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule_id == RuleId::PGP104)
        .expect("disconnected relation groups should emit PGP104");
    let evidence = serde_json::to_string(&diagnostic.evidence).unwrap();
    assert!(!evidence.contains("pgpreflight-secret-literal"));
    assert!(!evidence.contains("SELECT"));

    let connected = parse_and_validate(
        "SELECT * \
         FROM public.pgpreflight_cartesian_left AS l \
         CROSS JOIN public.pgpreflight_cartesian_right AS r \
         WHERE l.id = r.id",
    )
    .unwrap();
    let connected_plan = planner.plan(&connected, &config.postgres).await.unwrap();
    let connected_report = analyze(connected_plan.analysis_input(), &config);

    assert!(
        connected_report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule_id != RuleId::PGP104)
    );

    admin
        .batch_execute(
            "DROP TABLE pgpreflight_cartesian_left;\
             DROP TABLE pgpreflight_cartesian_right;",
        )
        .await
        .unwrap();
}
