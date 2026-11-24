use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::state::AppState;
use memo::dir::DirMeta;
use memo::file::{FileDto, FileStatus};
use memo::job::{JobDto, NewJobDto};
use snafu::ResultExt;

pub(super) const LOGGER_JOB_TYPE: &str = "logger_job_handler";
pub(super) const CREATE_IMAGE_VERSIONS_JOB_TYPE: &str = "create_image_versions";
pub(super) const DELETE_FILE_JOB_TYPE: &str = "delete_file";
pub(super) const CLEANUP_FILE_JOB_TYPE: &str = "cleanup_file";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CleanupFilePayload {
    file_id: String,
}

pub fn cleanup_file_job(file_id: String) -> NewJobDto {
    NewJobDto {
        job_type: CLEANUP_FILE_JOB_TYPE.to_string(),
        payload: serde_json::to_string(&CleanupFilePayload { file_id })
            .expect("cleanup file payload must serialize"),
        max_attempts: 3,
        available_at: chrono::Utc::now().timestamp(),
    }
}

async fn cleanup_file_job_handler(state: &AppState, file_id: &str) -> crate::Result<()> {
    let cutoff = chrono::Utc::now().timestamp().saturating_sub(60 * 60);
    let Some(file) = state
        .db
        .files
        .get(file_id)
        .await
        .context(crate::error::DbSnafu)?
    else {
        return Ok(());
    };
    if file.created_at > cutoff {
        return Ok(());
    }
    if file.status == FileStatus::Processing {
        if !state
            .db
            .files
            .claim_processing_cleanup(file_id, cutoff)
            .await
            .context(crate::error::DbSnafu)?
        {
            return Ok(());
        }
        state.file_cache.remove(file_id);
    } else if file.status != FileStatus::Cleaning {
        return Ok(());
    }

    if let Some(dir) = state
        .db
        .dirs
        .get(&file.dir_id)
        .await
        .context(crate::error::DbSnafu)?
    {
        let dir_meta = DirMeta {
            bucket_name: state.config.cloud.bucket.clone(),
            org_id: dir.org_id,
            dir_type: dir.dir_type,
            dir_name: dir.name,
        };
        if let Err(error) = state
            .storage_client
            .delete_processing_file(&dir_meta, &file)
            .await
        {
            warn!(%error, %file_id, "Unable to delete all processing file objects");
        }
    } else {
        warn!(%file_id, "Cannot clean up storage objects: directory not found");
    }
    state
        .db
        .files
        .delete_cleaning(file_id)
        .await
        .context(crate::error::DbSnafu)?;
    state.file_cache.remove(file_id);
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteFilePayload {
    file: FileDto,
    dir: DirMeta,
}

pub fn delete_file_job(file: FileDto, dir: DirMeta) -> NewJobDto {
    NewJobDto {
        job_type: DELETE_FILE_JOB_TYPE.to_string(),
        payload: serde_json::to_string(&DeleteFilePayload { file, dir })
            .expect("delete file payload must serialize"),
        max_attempts: 3,
        available_at: chrono::Utc::now().timestamp(),
    }
}

async fn delete_file_job_handler(
    state: &AppState,
    payload: &DeleteFilePayload,
) -> crate::Result<()> {
    info!("Deleting file: {}", &payload.file.id);
    state
        .db
        .files
        .delete(&payload.file.id)
        .await
        .context(crate::error::DbSnafu)?;
    state.file_cache.remove(&payload.file.id);
    state
        .storage_client
        .delete(&payload.dir, &payload.file)
        .await
        .context(crate::error::StorageSnafu)?;
    Ok(())
}

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
    info!("Creating image versions for file: {}", file_id);
    crate::file::process_image_svc(state, file_id).await
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoggerJobPayload {
    pub message: String,
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
        DELETE_FILE_JOB_TYPE => {
            let payload: DeleteFilePayload = serde_json::from_str(&job.payload)
                .map_err(|_| "Invalid delete file job payload".to_string())?;
            delete_file_job_handler(state, &payload)
                .await
                .map_err(|error| error.to_string())
        }
        CLEANUP_FILE_JOB_TYPE => {
            let payload: CleanupFilePayload = serde_json::from_str(&job.payload)
                .map_err(|_| "Invalid cleanup file job payload".to_string())?;
            cleanup_file_job_handler(state, &payload.file_id)
                .await
                .map_err(|error| error.to_string())
        }
        _ => Err("Unknown job type".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memo::dir::DirType;
    use memo::file::{FileStatus, FileType};

    #[test]
    fn image_job_has_expected_payload_and_defaults() {
        let job = create_image_versions_job("file-123".to_string());
        let payload: CreateImageVersionsPayload = serde_json::from_str(&job.payload).unwrap();
        assert_eq!(job.job_type, "create_image_versions");
        assert_eq!(payload.file_id, "file-123");
        assert_eq!(job.max_attempts, 3);
    }

    #[test]
    fn cleanup_job_has_expected_payload_and_defaults() {
        let job = cleanup_file_job("file-123".to_string());
        let payload: CleanupFilePayload = serde_json::from_str(&job.payload).unwrap();
        assert_eq!(job.job_type, "cleanup_file");
        assert_eq!(payload.file_id, "file-123");
        assert_eq!(job.max_attempts, 3);
    }

    #[test]
    fn delete_job_preserves_storage_metadata() {
        let file = FileDto {
            id: "file-123".into(),
            org_id: "org-123".into(),
            dir_id: "dir-123".into(),
            file_type: FileType::File,
            status: FileStatus::Inactive,
            name: "example.txt".into(),
            filename: "stored.txt".into(),
            content_type: "text/plain".into(),
            size: 10,
            url: None,
            img_versions: None,
            img_taken_at: None,
            created_at: 1,
            updated_at: 2,
        };
        let dir = DirMeta {
            bucket_name: "bucket".into(),
            org_id: "org-123".into(),
            dir_type: DirType::Documents,
            dir_name: "documents".into(),
        };
        let job = delete_file_job(file, dir);
        let payload: DeleteFilePayload = serde_json::from_str(&job.payload).unwrap();
        assert_eq!(job.job_type, "delete_file");
        assert_eq!(job.max_attempts, 3);
        assert_eq!(payload.file.id, "file-123");
        assert_eq!(payload.file.filename, "stored.txt");
        assert_eq!(payload.dir.bucket_name, "bucket");
        assert_eq!(payload.dir.dir_type, DirType::Documents);
    }
}
