use clap::Parser;
use config::CliArgs;
use run::run_command;
use snafu::ErrorCompat;
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
pub use error::{Error2, Result2};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let args = CliArgs::parse();

    if let Err(e) = run_command(args).await {
        eprintln!("Application error: {}", e);
        if let Some(bt) = ErrorCompat::backtrace(&e) {
            eprintln!("{:?}", bt);
        }
        process::exit(1);
    }
}
