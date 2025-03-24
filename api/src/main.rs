use clap::Parser;
use config::CliArgs;
use run::run_command;
use std::process;

mod auth;
mod bucket;
mod client;
mod command;
mod config;
mod db;
mod dir;
mod error;
mod file;
mod health;
mod run;
mod schema;
mod storage;
mod web;

// Re-export error types for convenience
pub use error::{Error, Error2, Result, Result2};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let args = CliArgs::parse();

    if let Err(e) = run_command(args).await {
        eprintln!("Application error: {e}");
        process::exit(1);
    }
}
