use run::run_command;
use std::{process, str::FromStr};
use tracing::Level;

mod auth;
mod bucket;
mod config;
mod dir;
mod error;
mod file;
mod health;
mod run;
mod state;
mod token;
mod web;

// Re-export error types for convenience
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
        eprintln!("Application error: {}", e);
        process::exit(1);
    }
}
