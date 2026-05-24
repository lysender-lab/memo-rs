use serde::Deserialize;
use snafu::{ResultExt, ensure};
use std::{env, path::PathBuf};

use crate::error::{ConfigSnafu, UploadDirSnafu};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub jwt_secret: String,
    pub upload_dir: PathBuf,
    pub cloud: CloudConfig,
    pub server: ServerConfig,
    pub db: DbConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone)]
pub struct CloudConfig {
    pub project_id: String,
    pub credentials: String,
    pub bucket: String,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub address: String,
}

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub dir: PathBuf,
    pub pool_size: usize,
}

const DEFAULT_DB_POOL_SIZE: usize = 4;

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub api_url: String,
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

        let pool_size = optional_env("DATABASE_POOL_SIZE")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(DEFAULT_DB_POOL_SIZE);

        let config = Config {
            jwt_secret: required_env("JWT_SECRET")?,
            upload_dir: PathBuf::from(required_env("UPLOAD_DIR")?),
            cloud: CloudConfig {
                project_id: required_env("GOOGLE_PROJECT_ID")?,
                credentials: required_env("GOOGLE_APPLICATION_CREDENTIALS")?,
                bucket: required_env("GOOGLE_STORAGE_BUCKET")?,
            },
            server: ServerConfig {
                address: required_env("SERVER_ADDRESS")?,
            },
            db: DbConfig {
                dir: db_dir,
                pool_size,
            },
            auth: AuthConfig {
                api_url: required_env("AUTH_API_BASE_URL")?,
            },
        };

        // Validate config values
        ensure!(
            !config.jwt_secret.is_empty(),
            ConfigSnafu {
                msg: "Jwt secret is required.".to_string()
            }
        );

        ensure!(
            !config.cloud.project_id.is_empty(),
            ConfigSnafu {
                msg: "Google Cloud Project ID is required.".to_string()
            }
        );

        ensure!(
            !config.cloud.credentials.is_empty(),
            ConfigSnafu {
                msg: "Google Cloud credentials file is required.".to_string()
            }
        );

        ensure!(
            config.upload_dir.exists(),
            ConfigSnafu {
                msg: "Upload directory does not exist.".to_string()
            }
        );

        let upload_dir = config.upload_dir.clone().join("tmp");
        std::fs::create_dir_all(&upload_dir).context(UploadDirSnafu)?;

        Ok(config)
    }
}

fn required_env(name: &str) -> Result<String> {
    match env::var(name) {
        Ok(val) => Ok(val),
        Err(_) => Err(Error::Config {
            msg: format!("{} is required.", name),
        }),
    }
}

fn optional_env(name: &str) -> Option<String> {
    match env::var(name) {
        Ok(val) if !val.trim().is_empty() => Some(val),
        _ => None,
    }
}
