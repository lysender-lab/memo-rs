use std::{process, str::FromStr};
use tracing::Level;

mod config;
mod ctx;
mod error;
mod models;
mod run;
mod services;
mod web;

use config::Config;
use run::run;

// Re-exports
pub use error::{Error, Result};

#[tokio::main]
async fn main() {
    let mut max_log = Level::INFO;
    if let Some(rust_log_max) = std::env::var_os("RUST_LOG_MAX") {
        max_log = Level::from_str(rust_log_max.to_str().unwrap()).unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
            process::exit(1);
        });
    }

    tracing_subscriber::fmt()
        .with_max_level(max_log)
        .with_target(false)
        .compact()
        .init();

    if let Err(e) = run_command().await {
        eprintln!("Application error: {e}");
        process::exit(1);
    }
}

async fn run_command() -> Result<()> {
    let config = Config::build_from_env()?;
    run(config).await
}
