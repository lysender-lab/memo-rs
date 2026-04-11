use snafu::ErrorCompat;
use std::process;

mod config;
mod error;
mod run;

use run::run;

// Re-export error types for convenience
pub use error::{Error, Result};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    if let Err(e) = run().await {
        eprintln!("Application error: {}", e);
        if let Some(bt) = ErrorCompat::backtrace(&e) {
            println!("{}", bt);
        }
        process::exit(1);
    }
}
