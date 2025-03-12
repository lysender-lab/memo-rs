use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{fs, path::PathBuf};

use crate::Result;

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
        let toml_string = match fs::read_to_string(filename) {
            Ok(str) => str,
            Err(e) => {
                return Err(format!("Error reading config file: {}", e).into());
            }
        };

        let config: Config = match toml::from_str(toml_string.as_str()) {
            Ok(value) => value,
            Err(e) => {
                return Err(format!("Error parsing config file: {}", e).into());
            }
        };

        // Validate config values
        if config.jwt_secret.len() == 0 {
            return Err("JWT secret is required.".into());
        }
        if config.cloud.project_id.len() == 0 {
            return Err("Google Cloud Project ID is required.".into());
        }
        if config.cloud.credentials.len() == 0 {
            return Err("Google Cloud credentials file is required.".into());
        }
        if config.db.url.len() == 0 {
            return Err("Database URL required.".into());
        }
        if config.server.port == 0 {
            return Err("PORT is required.".into());
        }

        let mut upload_dir = config.upload_dir.clone();
        if !upload_dir.exists() {
            return Err("Upload directory does not exist.".into());
        }
        upload_dir = upload_dir.join("tmp");
        std::fs::create_dir_all(&upload_dir).expect("Unable to create upload tmp dir");

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

    /// Manages clients
    #[command(subcommand)]
    Clients(ClientCommand),

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
pub enum ClientCommand {
    List,
    Create { name: String },
    Enable { id: String },
    Disable { id: String },
    Delete { id: String },
    SetDefaultBucket { id: String, bucket_id: String },
    UnsetDefaultBucket { id: String },
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
