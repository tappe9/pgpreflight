use std::{env, fs, path::Path};

use pgpreflight_core::Config;

const CONFIG_FILE_NAME: &str = "pgpreflight.toml";
const DATABASE_ENV_VARS: [&str; 2] = ["PGPREFLIGHT_DATABASE_URL", "DATABASE_URL"];

#[derive(Clone, Copy)]
pub(crate) enum ConfigFailure {
    Io,
    Parse,
}

#[derive(Clone, Copy)]
pub(crate) enum DatabaseUrlFailure {
    Missing,
    Invalid,
}

pub(crate) fn load_config(explicit: Option<&Path>) -> Result<Config, ConfigFailure> {
    let config = match explicit {
        Some(path) => parse_config(&fs::read(path).map_err(|_| ConfigFailure::Io)?)?,
        None => discover_config()?,
    };
    config.validate().map_err(|_| ConfigFailure::Parse)?;
    Ok(config)
}

fn discover_config() -> Result<Config, ConfigFailure> {
    let current_dir = env::current_dir().map_err(|_| ConfigFailure::Io)?;

    for directory in current_dir.ancestors() {
        let path = directory.join(CONFIG_FILE_NAME);
        match fs::read(path) {
            Ok(bytes) => return parse_config(&bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ConfigFailure::Io),
        }
    }

    Ok(Config::default())
}

fn parse_config(bytes: &[u8]) -> Result<Config, ConfigFailure> {
    let source = std::str::from_utf8(bytes).map_err(|_| ConfigFailure::Parse)?;
    toml::from_str(source).map_err(|_| ConfigFailure::Parse)
}

pub(crate) fn resolve_database_url(explicit: Option<String>) -> Result<String, DatabaseUrlFailure> {
    if let Some(database_url) = explicit {
        return validate_database_url(database_url);
    }

    for variable in DATABASE_ENV_VARS {
        if let Some(value) = env::var_os(variable) {
            let database_url = value
                .into_string()
                .map_err(|_| DatabaseUrlFailure::Invalid)?;
            return validate_database_url(database_url);
        }
    }

    Err(DatabaseUrlFailure::Missing)
}

fn validate_database_url(database_url: String) -> Result<String, DatabaseUrlFailure> {
    if database_url.is_empty() {
        Err(DatabaseUrlFailure::Invalid)
    } else {
        Ok(database_url)
    }
}
