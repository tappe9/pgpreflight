use pgpreflight_core::{PlanNodeKind, PostgresConfig, StatementKind};
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
async fn supported_postgresql_major_preserves_semantic_safe_mode_contracts() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let admin = connect_test_client(&database_url).await;
    let version_text: String = admin
        .query_one("SELECT current_setting('server_version_num')", &[])
        .await
        .expect("query server version")
        .get(0);
    let version_number = version_text
        .parse::<u32>()
        .expect("server_version_num should be numeric");
    let major = version_number / 10_000;

    assert!(
        (14..=18).contains(&major),
        "unexpected PostgreSQL major: {major}"
    );
    if let Ok(expected) = std::env::var("PGPREFLIGHT_TEST_POSTGRES_MAJOR") {
        assert_eq!(
            major,
            expected
                .parse::<u32>()
                .expect("expected PostgreSQL major should be numeric")
        );
    }

    let table = format!("pgpreflight_version_probe_{}", std::process::id());
    admin
        .batch_execute(&format!(
            "DROP TABLE IF EXISTS public.{table}; \
             CREATE TABLE public.{table} (id integer PRIMARY KEY, payload text NOT NULL); \
             INSERT INTO public.{table} VALUES (1, 'before'); \
             ANALYZE public.{table};"
        ))
        .await
        .expect("prepare version probe table");

    let sql = format!("UPDATE public.{table} SET payload = 'version-secret-marker' WHERE id = 1");
    let statement = parse_and_validate(&sql).expect("version probe SQL should be supported");
    let mut planner = SafeModePlanner::connect(&database_url)
        .await
        .expect("connect Safe Mode planner");
    let planned = planner
        .plan(&statement, &PostgresConfig::default())
        .await
        .expect("plan version probe SQL");
    let input = planned.analysis_input();

    assert_eq!(input.statement.kind, StatementKind::Update);
    assert_eq!(input.plan.root.kind, PlanNodeKind::ModifyTable);
    assert!(
        input
            .plan
            .estimated_affected_rows
            .is_some_and(|rows| rows.is_finite() && rows > 0.0)
    );
    let stats = input
        .relations
        .iter()
        .find(|stats| stats.relation.name == table)
        .expect("probe relation statistics should be normalized");
    assert!(
        stats
            .estimated_live_rows
            .is_some_and(|rows| rows.is_finite() && rows > 0.0)
    );
    assert!(stats.pages.is_some());

    let serialized = serde_json::to_string(input).expect("serialize normalized analysis input");
    assert!(!serialized.contains("version-secret-marker"));

    let payload: String = admin
        .query_one(
            &format!("SELECT payload FROM public.{table} WHERE id = 1"),
            &[],
        )
        .await
        .expect("query version probe row")
        .get(0);
    assert_eq!(payload, "before", "plain EXPLAIN must not execute UPDATE");

    admin
        .batch_execute(&format!("DROP TABLE public.{table}"))
        .await
        .expect("drop version probe table");
}
