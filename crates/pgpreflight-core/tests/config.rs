use pgpreflight_core::Config;

#[test]
fn default_thresholds_match_v01_contract() {
    let config = Config::default();

    assert_eq!(config.version, 1);
    assert_eq!(config.postgres.statement_timeout_ms, 3_000);
    assert_eq!(config.postgres.lock_timeout_ms, 500);
    assert_eq!(config.rules.pgp101.max_rows, 10_000.0);
    assert_eq!(config.rules.pgp101.max_table_ratio, 0.05);
    assert_eq!(config.rules.pgp101.min_rows_for_ratio, 1_000.0);
    assert_eq!(config.rules.pgp102.min_relation_rows, 100_000.0);
    assert_eq!(config.rules.pgp102.max_output_ratio, 0.20);
    assert_eq!(config.rules.pgp103.max_result_rows, 100_000.0);
}

#[test]
fn unknown_config_key_is_rejected() {
    let text = "version = 1\nunknown = true\n";
    let result = toml::from_str::<Config>(text);

    assert!(result.is_err());
}

#[test]
fn unsupported_config_version_fails_validation() {
    let text = "version = 2\n";
    let config = toml::from_str::<Config>(text).expect("version itself should deserialize");

    assert!(config.validate().is_err());
}

#[test]
fn invalid_ratios_fail_validation() {
    let text = r#"
version = 1

[rules.PGP101]
max_table_ratio = 1.1
"#;
    let config = toml::from_str::<Config>(text).expect("ratio should deserialize before validation");

    assert!(config.validate().is_err());
}
