use pgpreflight_core::{PlanNodeKind, RelationRef, StatementKind};
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
async fn planner_returns_normalized_semantic_plan_and_catalog_stats() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let admin = connect_test_client(&database_url).await;
    admin
        .batch_execute(
            "DROP TABLE IF EXISTS pgpreflight_normalization_probe;\
             CREATE TABLE pgpreflight_normalization_probe (id integer PRIMARY KEY, payload text NOT NULL);\
             INSERT INTO pgpreflight_normalization_probe \
             SELECT value, 'payload-' || value::text FROM generate_series(1, 200) AS value;\
             ANALYZE pgpreflight_normalization_probe;",
        )
        .await
        .unwrap();

    let mut planner = SafeModePlanner::connect(&database_url).await.unwrap();
    let statement = parse_and_validate(
        "SELECT * FROM pgpreflight_normalization_probe \
         WHERE payload <> 'integration-secret-marker'",
    )
    .unwrap();
    let planned = planner
        .plan(&statement, &pgpreflight_core::PostgresConfig::default())
        .await
        .unwrap();
    let input = planned.analysis_input();

    assert_eq!(input.statement.kind, StatementKind::Select);
    assert_eq!(input.plan.root.kind, PlanNodeKind::SeqScan);
    assert_eq!(
        input.plan.root.relation,
        Some(RelationRef::new(
            "public",
            "pgpreflight_normalization_probe"
        ))
    );
    assert!(input.plan.root.estimated_rows >= 0.0);
    assert!(input.plan.root.total_cost >= input.plan.root.startup_cost);

    let stats = input
        .relations
        .iter()
        .find(|stats| stats.relation.name == "pgpreflight_normalization_probe")
        .unwrap();
    assert!(stats.estimated_live_rows.is_some_and(|rows| rows > 0.0));
    assert!(stats.pages.is_some());

    let serialized = serde_json::to_string(input).unwrap();
    assert!(!serialized.contains("integration-secret-marker"));
    assert!(!serialized.contains("Filter"));

    admin
        .batch_execute("DROP TABLE pgpreflight_normalization_probe")
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn catalog_stats_preserve_unknown_live_row_estimate() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let admin = connect_test_client(&database_url).await;
    admin
        .batch_execute(
            "DROP TABLE IF EXISTS pgpreflight_unanalyzed_probe;\
             CREATE TABLE pgpreflight_unanalyzed_probe (id integer);",
        )
        .await
        .unwrap();

    let mut planner = SafeModePlanner::connect(&database_url).await.unwrap();
    let statement = parse_and_validate("SELECT * FROM pgpreflight_unanalyzed_probe").unwrap();
    let planned = planner
        .plan(&statement, &pgpreflight_core::PostgresConfig::default())
        .await
        .unwrap();
    let stats = planned
        .analysis_input()
        .relations
        .iter()
        .find(|stats| stats.relation.name == "pgpreflight_unanalyzed_probe")
        .unwrap();

    assert_eq!(stats.estimated_live_rows, None);

    admin
        .batch_execute("DROP TABLE pgpreflight_unanalyzed_probe")
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn update_uses_conservative_affected_row_estimate() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let admin = connect_test_client(&database_url).await;
    admin
        .batch_execute(
            "DROP TABLE IF EXISTS pgpreflight_affected_probe;\
             CREATE TABLE pgpreflight_affected_probe (id integer PRIMARY KEY, payload text NOT NULL);\
             INSERT INTO pgpreflight_affected_probe \
             SELECT value, 'before' FROM generate_series(1, 100) AS value;\
             ANALYZE pgpreflight_affected_probe;",
        )
        .await
        .unwrap();

    let mut planner = SafeModePlanner::connect(&database_url).await.unwrap();
    let statement = parse_and_validate(
        "UPDATE pgpreflight_affected_probe SET payload = 'after' WHERE id <= 10",
    )
    .unwrap();
    let planned = planner
        .plan(&statement, &pgpreflight_core::PostgresConfig::default())
        .await
        .unwrap();
    let input = planned.analysis_input();

    assert_eq!(input.statement.kind, StatementKind::Update);
    assert_eq!(input.plan.root.kind, PlanNodeKind::ModifyTable);
    assert!(
        input
            .plan
            .estimated_affected_rows
            .is_some_and(|rows| rows > 0.0)
    );

    admin
        .batch_execute("DROP TABLE pgpreflight_affected_probe")
        .await
        .unwrap();
}
