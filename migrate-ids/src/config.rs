use snafu::ensure;
use std::{env, path::PathBuf};

use crate::Result;
use crate::error::ConfigSnafu;

#[derive(Debug, Clone)]
pub struct Config {
    pub db_dir: PathBuf,
}

impl Config {
    pub fn build() -> Result<Self> {
        let db_dir = PathBuf::from(required_env("DATABASE_DIR")?);

        ensure!(
            db_dir.exists(),
            ConfigSnafu {
                msg: "Database directory does not exist.".to_string()
            }
        );

        Ok(Config { db_dir })
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
