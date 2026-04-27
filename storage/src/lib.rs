mod client;
mod error;
mod provider;
mod providers;

// Re-export error types for convenience
pub use error::{Error, Result};

pub use client::StorageClient;
pub use provider::DownloadedFile;
