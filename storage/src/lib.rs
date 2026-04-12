pub mod error;
pub mod storage;

// Re-export error types for convenience
pub use error::{Error, Result};

pub use storage::StorageClient;
