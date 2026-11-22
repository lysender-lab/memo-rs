use serde::{Deserialize, Serialize};
use tracing::info;

use crate::state::AppState;
use memo::job::{JobDto, NewJobDto};

pub(super) const LOGGER_JOB_TYPE: &str = "logger_job_handler";
pub(super) const CREATE_IMAGE_VERSIONS_JOB_TYPE: &str = "create_image_versions";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateImageVersionsPayload {
    file_id: String,
}

pub fn create_image_versions_job(file_id: String) -> NewJobDto {
    NewJobDto {
        job_type: CREATE_IMAGE_VERSIONS_JOB_TYPE.to_string(),
        payload: serde_json::to_string(&CreateImageVersionsPayload { file_id })
            .expect("image job payload must serialize"),
        max_attempts: 3,
        available_at: chrono::Utc::now().timestamp(),
    }
}

async fn create_image_versions_job_handler(state: &AppState, file_id: &str) -> crate::Result<()> {
    crate::file::process_image_svc(state, file_id).await
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoggerJobPayload {
    pub message: String,
}

/// Create a logger job available for immediate processing.
pub fn logger_job(message: String) -> NewJobDto {
    let payload = LoggerJobPayload { message };
    NewJobDto {
        job_type: LOGGER_JOB_TYPE.to_string(),
        payload: serde_json::to_string(&payload).expect("logger payload must serialize"),
        max_attempts: 3,
        available_at: chrono::Utc::now().timestamp(),
    }
}

async fn logger_job_handler(message: &str) {
    info!("{}", message);
}

pub(super) async fn dispatch(state: &AppState, job: &JobDto) -> std::result::Result<(), String> {
    match job.job_type.as_str() {
        LOGGER_JOB_TYPE => {
            let payload: LoggerJobPayload = serde_json::from_str(&job.payload)
                .map_err(|_| "Invalid logger job payload".to_string())?;
            logger_job_handler(&payload.message).await;
            Ok(())
        }
        CREATE_IMAGE_VERSIONS_JOB_TYPE => {
            let payload: CreateImageVersionsPayload = serde_json::from_str(&job.payload)
                .map_err(|_| "Invalid image job payload".to_string())?;
            create_image_versions_job_handler(state, &payload.file_id)
                .await
                .map_err(|error| error.to_string())
        }
        _ => Err("Unknown job type".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logger_job_has_expected_payload_and_defaults() {
        let before = chrono::Utc::now().timestamp();
        let job = logger_job("hello".to_string());
        let after = chrono::Utc::now().timestamp();
        let payload: LoggerJobPayload = serde_json::from_str(&job.payload).unwrap();

        assert_eq!(job.job_type, LOGGER_JOB_TYPE);
        assert_eq!(payload.message, "hello");
        assert_eq!(job.max_attempts, 3);
        assert!((before..=after).contains(&job.available_at));
    }

    #[test]
    fn image_job_has_expected_payload_and_defaults() {
        let job = create_image_versions_job("file-123".to_string());
        let payload: CreateImageVersionsPayload = serde_json::from_str(&job.payload).unwrap();
        assert_eq!(job.job_type, "create_image_versions");
        assert_eq!(payload.file_id, "file-123");
        assert_eq!(job.max_attempts, 3);
    }
}
