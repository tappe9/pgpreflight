use pgpreflight_core::{Config, DiagnosticEvidence, PlanNodeKind, RelationRef, RuleId, analyze};
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
async fn normalized_planner_evidence_drives_scan_and_result_rules() {
    let Some(database_url) = test_database_url() else {
        return;
    };
    let admin = connect_test_client(&database_url).await;
    admin
        .batch_execute(
            "DROP TABLE IF EXISTS pgpreflight_scan_result_probe;\
             CREATE TABLE pgpreflight_scan_result_probe (id integer, payload text NOT NULL);\
             INSERT INTO pgpreflight_scan_result_probe \
             SELECT value, 'payload-' || value::text FROM generate_series(1, 200) AS value;\
             ANALYZE pgpreflight_scan_result_probe;",
        )
        .await
        .unwrap();

    let mut planner = SafeModePlanner::connect(&database_url).await.unwrap();
    let statement =
        parse_and_validate("SELECT * FROM pgpreflight_scan_result_probe LIMIT 5").unwrap();
    let planned = planner
        .plan(&statement, &pgpreflight_core::PostgresConfig::default())
        .await
        .unwrap();
    let input = planned.analysis_input();

    assert_eq!(input.plan.root.kind, PlanNodeKind::Limit);
    assert_eq!(input.plan.root.children.len(), 1);
    assert_eq!(input.plan.root.children[0].kind, PlanNodeKind::SeqScan);

    let mut config = Config::default();
    config.rules.pgp102.min_relation_rows = 1.0;
    config.rules.pgp102.max_output_ratio = 1.0;
    config.rules.pgp103.max_result_rows = 5.0;

    let report = analyze(input, &config);

    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.rule_id)
            .collect::<Vec<_>>(),
        vec![RuleId::PGP102, RuleId::PGP103]
    );

    let scan = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule_id == RuleId::PGP102)
        .unwrap();
    match &scan.evidence {
        DiagnosticEvidence::LargeSequentialScan {
            relation,
            estimated_scanned_rows,
            estimated_output_rows,
            estimated_output_ratio,
            ..
        } => {
            assert_eq!(
                relation,
                &RelationRef::new("public", "pgpreflight_scan_result_probe")
            );
            assert!(*estimated_scanned_rows > 0.0);
            assert!(*estimated_output_rows > 0.0);
            assert!(*estimated_output_ratio <= 1.0);
        }
        other => panic!("unexpected scan evidence: {other:?}"),
    }

    let result = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule_id == RuleId::PGP103)
        .unwrap();
    assert_eq!(
        result.evidence,
        DiagnosticEvidence::LargeResultSet {
            estimated_result_rows: input.plan.root.estimated_rows,
        }
    );

    admin
        .batch_execute("DROP TABLE pgpreflight_scan_result_probe")
        .await
        .unwrap();
}
