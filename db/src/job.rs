use std::sync::Arc;
use std::time::Duration;

use memo::job::{JobDto, NewJobDto};

use crate::Result;
use crate::db_pool::DbPool;

pub struct JobRepo {
    #[allow(dead_code)]
    db_pool: Arc<DbPool>,
}

impl JobRepo {
    pub(crate) fn new(db_pool: Arc<DbPool>) -> Self {
        Self { db_pool }
    }

    /// Enqueue a new pending job.
    pub async fn enqueue(&self, _input: NewJobDto) -> Result<JobDto> {
        todo!()
    }

    /// Atomically claim the next eligible job, or return None if none is available.
    pub async fn claim_next(&self, _lease_duration: Duration) -> Result<Option<JobDto>> {
        todo!()
    }

    /// Complete a job if the lease token still matches and has not expired.
    pub async fn complete(&self, _job_id: &str, _lease_token: &str) -> Result<bool> {
        todo!()
    }

    /// Retry at the specified Unix timestamp, or mark failed if attempts are exhausted.
    /// Returns false if the lease token no longer matches or has expired.
    pub async fn fail(
        &self,
        _job_id: &str,
        _lease_token: &str,
        _error: &str,
        _retry_at: i64,
    ) -> Result<bool> {
        todo!()
    }

    /// Extend an unexpired lease from the current time.
    pub async fn extend_lease(
        &self,
        _job_id: &str,
        _lease_token: &str,
        _lease_duration: Duration,
    ) -> Result<bool> {
        todo!()
    }

    /// Recover expired leases, returning the number of jobs changed.
    pub async fn recover_expired(&self) -> Result<u64> {
        todo!()
    }

    /// Delete completed and failed jobs updated before the Unix timestamp cutoff.
    pub async fn delete_finished_before(&self, _cutoff: i64) -> Result<u64> {
        todo!()
    }

    /// Get a job by ID.
    pub async fn get(&self, _job_id: &str) -> Result<Option<JobDto>> {
        todo!()
    }
}
