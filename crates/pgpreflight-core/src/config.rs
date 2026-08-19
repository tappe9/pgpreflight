use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub postgres: PostgresConfig,
    pub rules: RulesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            postgres: PostgresConfig::default(),
            rules: RulesConfig::default(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != 1 {
            return Err(ConfigError::UnsupportedVersion(self.version));
        }
        if !(0.0..=1.0).contains(&self.rules.pgp101.max_table_ratio) {
            return Err(ConfigError::InvalidRatio("rules.PGP101.max_table_ratio"));
        }
        if !(0.0..=1.0).contains(&self.rules.pgp102.max_output_ratio) {
            return Err(ConfigError::InvalidRatio("rules.PGP102.max_output_ratio"));
        }
        if self.rules.pgp101.max_rows < 0.0
            || self.rules.pgp101.min_rows_for_ratio < 0.0
            || self.rules.pgp102.min_relation_rows < 0.0
            || self.rules.pgp103.max_result_rows < 0.0
        {
            return Err(ConfigError::NegativeThreshold);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PostgresConfig {
    pub statement_timeout_ms: u64,
    pub lock_timeout_ms: u64,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            statement_timeout_ms: 3_000,
            lock_timeout_ms: 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct RulesConfig {
    #[serde(rename = "PGP001")]
    pub pgp001: ToggleRuleConfig,
    #[serde(rename = "PGP002")]
    pub pgp002: ToggleRuleConfig,
    #[serde(rename = "PGP101")]
    pub pgp101: LargeAffectedConfig,
    #[serde(rename = "PGP102")]
    pub pgp102: LargeSequentialScanConfig,
    #[serde(rename = "PGP103")]
    pub pgp103: LargeResultConfig,
    #[serde(rename = "PGP104")]
    pub pgp104: ToggleRuleConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToggleRuleConfig {
    pub enabled: bool,
}

impl Default for ToggleRuleConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LargeAffectedConfig {
    pub enabled: bool,
    pub max_rows: f64,
    pub max_table_ratio: f64,
    pub min_rows_for_ratio: f64,
}

impl Default for LargeAffectedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_rows: 10_000.0,
            max_table_ratio: 0.05,
            min_rows_for_ratio: 1_000.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LargeSequentialScanConfig {
    pub enabled: bool,
    pub min_relation_rows: f64,
    pub max_output_ratio: f64,
}

impl Default for LargeSequentialScanConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_relation_rows: 100_000.0,
            max_output_ratio: 0.20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LargeResultConfig {
    pub enabled: bool,
    pub max_result_rows: f64,
}

impl Default for LargeResultConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_result_rows: 100_000.0,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("unsupported configuration version {0}")]
    UnsupportedVersion(u32),
    #[error("ratio must be between 0 and 1: {0}")]
    InvalidRatio(&'static str),
    #[error("numeric thresholds must not be negative")]
    NegativeThreshold,
}
