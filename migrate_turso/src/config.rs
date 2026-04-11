use clap::{Parser, Subcommand};
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
    pub url: String,
}

impl Config {
    pub fn build_from_env() -> Result<Self> {
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
            db: DbConfig {
                url: required_env("DATABASE_URL")?,
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
            !config.db.url.is_empty(),
            ConfigSnafu {
                msg: "Database URL is required.".to_string()
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

/// File Management in the cloud
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Runs the API server
    Server,

    /// Sets up the admin user
    Setup,
}
