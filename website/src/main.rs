mod config;
mod ctx;
mod error;
mod models;
mod run;
mod services;
mod web;

use clap::Parser;
use std::process;

use config::{Args, Config};
use run::run;

// Re-exports
pub use error::{Error, Result};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let args = Args::parse();

    if let Err(e) = run_command(args).await {
        eprintln!("Application error: {e}");
        process::exit(1);
    }
}

async fn run_command(arg: Args) -> Result<()> {
    let config = Config::build(&arg.config)?;
    run(config).await
}
