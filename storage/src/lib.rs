pub mod error;
pub mod storage;

// Re-export error types for convenience
pub use error::{Error, Result};

pub use storage::{CloudStorable, StorageClient};

#[cfg(feature = "test")]
pub use storage::StorageTestClient;
