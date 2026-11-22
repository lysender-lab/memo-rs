use memo::job::{JobDto, NewJobDto};
use serde::{Deserialize, Serialize};
use tracing::info;

pub(super) const LOGGER_JOB_TYPE: &str = "logger_job_handler";

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

pub(super) async fn dispatch(job: &JobDto) -> std::result::Result<(), String> {
    match job.job_type.as_str() {
        LOGGER_JOB_TYPE => {
            let payload: LoggerJobPayload = serde_json::from_str(&job.payload)
                .map_err(|_| "Invalid logger job payload".to_string())?;
            logger_job_handler(&payload.message).await;
            Ok(())
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
}
