use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDto {
    pub id: String,
    pub job_type: String,
    /// JSON-encoded job input.
    pub payload: String,
    pub status: JobStatus,
    /// Number of times the job has been claimed.
    pub attempts: i64,
    pub max_attempts: i64,
    /// Unix timestamps are expressed in seconds.
    pub available_at: i64,
    pub lease_token: Option<String>,
    pub lease_expires_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewJobDto {
    pub job_type: String,
    /// JSON-encoded job input.
    pub payload: String,
    pub max_attempts: i64,
    /// Unix timestamp in seconds; use the current time for an immediate job.
    pub available_at: i64,
}
