//! Database-backed job workers and job dispatch.

mod handlers;
mod worker;

use db::DbMapper;
use memo::job::{JobDto, NewJobDto};

pub use handlers::create_image_versions_job;
pub use worker::WorkerPool;

/// Enqueue a job for later processing.
pub async fn enqueue(db: &DbMapper, job: NewJobDto) -> db::Result<JobDto> {
    db.jobs.enqueue(job).await
}
