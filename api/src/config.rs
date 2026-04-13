use serde::Deserialize;
use snafu::{ResultExt, ensure};
use std::{env, path::PathBuf};

use crate::Result;
use crate::error::{ConfigSnafu, UploadDirSnafu};

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
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub address: String,
}

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub auth_url: String,
    pub api_url: String,
    pub client_id: String,
    pub client_secret: String,
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

        let config = Config {
            jwt_secret: required_env("JWT_SECRET")?,
            upload_dir: PathBuf::from(required_env("UPLOAD_DIR")?),
            cloud: CloudConfig {
                project_id: required_env("GOOGLE_PROJECT_ID")?,
                credentials: required_env("GOOGLE_APPLICATION_CREDENTIALS")?,
            },
            server: ServerConfig {
                address: required_env("SERVER_ADDRESS")?,
            },
            db: DbConfig { dir: db_dir },
            auth: AuthConfig {
                auth_url: required_env("AUTH_PUBLIC_BASE_URL")?,
                api_url: required_env("AUTH_API_BASE_URL")?,
                client_id: required_env("AUTH_CLIENT_ID")?,
                client_secret: required_env("AUTH_CLIENT_SECRET")?,
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
        Err(_) => ConfigSnafu {
            msg: format!("{} is required.", name),
        }
        .fail(),
    }
}
