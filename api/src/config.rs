use clap::{Parser, Subcommand};
use serde::Deserialize;
use snafu::{Backtrace, ResultExt, Snafu, ensure};
use std::{fs, path::PathBuf};

#[derive(Debug, Snafu)]
pub enum ConfigError {
    #[snafu(display("Error reading config file: {}", source))]
    ConfigFile {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Error parsing config file: {}", source))]
    ConfigParse {
        source: toml::de::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Config error: {}", msg))]
    Config { msg: String },

    #[snafu(display("Unable to create upload dir"))]
    UploadDir {
        source: std::io::Error,
        backtrace: Backtrace,
    },
}

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
    pub fn build(filename: &PathBuf) -> Result<Self, ConfigError> {
        let toml_string = fs::read_to_string(filename).context(ConfigFileSnafu)?;

        let config: Config = toml::from_str(toml_string.as_str()).context(ConfigParseSnafu)?;

        // Validate config values
        ensure!(
            config.jwt_secret.len() > 0,
            ConfigSnafu {
                msg: "JWT secret is required.".to_string()
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

        let mut upload_dir = config.upload_dir.clone();
        ensure!(
            upload_dir.exists(),
            ConfigSnafu {
                msg: "Upload directory must be an absolute path.".to_string()
            }
        );

        upload_dir = upload_dir.join("tmp");

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

    /// Manages client users
    #[command(subcommand)]
    Users(UserCommand),

    /// Manages client buckets
    #[command(subcommand)]
    Buckets(BucketCommand),

    /// Checks health of the API server
    CheckHealth,
}

#[derive(Subcommand, Debug)]
pub enum UserCommand {
    List {
        client_id: String,
    },
    Create {
        client_id: String,
        username: String,
        roles: String,
    },
    Password {
        id: String,
    },
    Enable {
        id: String,
    },
    Disable {
        id: String,
    },
    Delete {
        id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum BucketCommand {
    List {
        client_id: String,
    },
    Create {
        client_id: String,
        name: String,
        images_only: String,
    },
    Delete {
        id: String,
    },
}
