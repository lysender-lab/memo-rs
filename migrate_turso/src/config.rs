use snafu::ensure;
use std::{env, path::PathBuf};

use crate::Result;
use crate::error::ConfigSnafu;

#[derive(Debug, Clone)]
pub struct Config {
    pub source_db_path: PathBuf,
    pub target_db_path: PathBuf,
}

impl Config {
    pub fn build() -> Result<Self> {
        let config = Config {
            source_db_path: PathBuf::from(required_env("SOURCE_DB_PATH")?),
            target_db_path: PathBuf::from(required_env("TARGET_DB_PATH")?),
        };

        ensure!(
            config.source_db_path.exists(),
            ConfigSnafu {
                msg: "Source DB should exist.".to_string()
            }
        );

        ensure!(
            config.target_db_path.exists(),
            ConfigSnafu {
                msg: "Target DB should exist.".to_string()
            }
        );

        Ok(config)
    }
}

fn required_env(name: &str) -> Result<String> {
    match env::var(name) {
        Ok(val) => Ok(val),
        Err(_) => ConfigSnafu {
            msg: format!("{} is required.", name),
        }
        .fail(),
    }
}
