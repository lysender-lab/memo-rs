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
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub aws_region: String,
    pub aws_role_arn: String,
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
                aws_access_key_id: required_env("AWS_ACCESS_KEY_ID")?,
                aws_secret_access_key: required_env("AWS_SECRET_ACCESS_KEY")?,
                aws_region: required_env("AWS_REGION")?,
                aws_role_arn: required_env("AWS_ROLE_ARN")?,
                bucket: required_env("AWS_S3_BUCKET")?,
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
            !config.cloud.aws_access_key_id.is_empty(),
            ConfigSnafu {
                msg: "AWS access key id is required.".to_string()
            }
        );

        ensure!(
            !config.cloud.aws_secret_access_key.is_empty(),
            ConfigSnafu {
                msg: "AWS secret access key is required.".to_string()
            }
        );

        ensure!(
            !config.cloud.aws_region.is_empty(),
            ConfigSnafu {
                msg: "AWS region is required.".to_string()
            }
        );

        ensure!(
            !config.cloud.aws_role_arn.is_empty(),
            ConfigSnafu {
                msg: "AWS role ARN is required.".to_string()
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
