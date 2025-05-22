use clap::{Parser, Subcommand};
use serde::Deserialize;
use snafu::{ResultExt, ensure};
use std::{fs, path::PathBuf};

use crate::Result;
use crate::error::{ConfigFileSnafu, ConfigParseSnafu, ConfigSnafu, UploadDirSnafu};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub jwt_secret: String,
    pub upload_dir: PathBuf,
    pub cloud: CloudConfig,
    pub server: ServerConfig,
    pub db: DbConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CloudConfig {
    pub project_id: String,
    pub credentials: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbConfig {
    pub url: String,
}

impl Config {
    pub fn build(filename: &PathBuf) -> Result<Self> {
        let toml_string = fs::read_to_string(filename).context(ConfigFileSnafu)?;
        let config: Config = toml::from_str(toml_string.as_str()).context(ConfigParseSnafu)?;

        // Validate config values
        ensure!(
            config.jwt_secret.len() > 0,
            ConfigSnafu {
                msg: "Jwt secret is required.".to_string()
            }
        );

        ensure!(
            config.cloud.project_id.len() > 0,
            ConfigSnafu {
                msg: "Google Cloud Project ID is required.".to_string()
            }
        );

        ensure!(
            config.cloud.credentials.len() > 0,
            ConfigSnafu {
                msg: "Google Cloud credentials file is required.".to_string()
            }
        );

        ensure!(
            config.db.url.len() > 0,
            ConfigSnafu {
                msg: "Database URL is required.".to_string()
            }
        );

        ensure!(
            config.server.port > 0,
            ConfigSnafu {
                msg: "Server port is required.".to_string()
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

/// File Management in the cloud
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, value_name = "config.toml")]
    pub config: PathBuf,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Runs the API server
    Server,

    /// Sets up the admin user
    Setup,
}
