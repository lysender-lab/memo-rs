use std::sync::Arc;

use snafu::{ResultExt, ensure};
use turso::Row;

use crate::Result;
use crate::db_pool::DbPool;
use crate::error::{DbPrepareSnafu, DbStatementSnafu, ValidationSnafu};
use crate::turso_decode::{FromTursoRow, collect_row, row_integer, row_text};
use crate::turso_params::{integer_param, text_param};
use memo::file::{UploadDto, UploadStatus};

impl FromTursoRow for UploadDto {
    fn from_row(row: &Row) -> Result<Self> {
        let status = row_text(row, 8)?;
        Ok(Self {
            id: row_text(row, 0)?,
            org_id: row_text(row, 1)?,
            dir_id: row_text(row, 2)?,
            original_filename: row_text(row, 3)?,
            filename: row_text(row, 4)?,
            content_type: row_text(row, 5)?,
            size: row_integer(row, 6)?,
            storage_key: row_text(row, 7)?,
            status: UploadStatus::try_from(status.as_str())
                .map_err(|_| crate::Error::from(format!("Invalid upload status: {status}")))?,
            created_at: row_integer(row, 9)?,
            expires_at: row_integer(row, 10)?,
        })
    }
}

pub struct UploadRepo {
    db_pool: Arc<DbPool>,
}

impl UploadRepo {
    pub(crate) fn new(db_pool: Arc<DbPool>) -> Self {
        Self { db_pool }
    }

    pub async fn get(&self, id: &str) -> Result<Option<UploadDto>> {
        let conn = self.db_pool.acquire().await?;
        let sql = "SELECT id, org_id, dir_id, original_filename, filename, content_type, \
                   size, storage_key, status, created_at, expires_at \
                   FROM uploads WHERE id = :id";
        let mut stmt = conn.prepare(sql).await.context(DbPrepareSnafu)?;
        collect_row(stmt.query_row(vec![text_param(":id", id.to_owned())]).await)
    }

    pub async fn reserve(&self, upload: &UploadDto) -> Result<()> {
        ensure!(
            upload.status == UploadStatus::Pending
                && upload.size >= 0
                && upload.expires_at > upload.created_at
                && !upload.storage_key.is_empty(),
            ValidationSnafu {
                msg: "Invalid upload reservation"
            }
        );

        let conn = self.db_pool.acquire().await?;
        let sql = "INSERT INTO uploads (id, org_id, dir_id, original_filename, filename, \
                   content_type, size, storage_key, status, created_at, expires_at) \
                   VALUES (:id, :org_id, :dir_id, :original_filename, :filename, \
                   :content_type, :size, :storage_key, 'pending', :created_at, :expires_at)";

        let mut stmt = conn.prepare(sql).await.context(DbPrepareSnafu)?;

        stmt.execute(vec![
            text_param(":id", upload.id.clone()),
            text_param(":org_id", upload.org_id.clone()),
            text_param(":dir_id", upload.dir_id.clone()),
            text_param(":original_filename", upload.original_filename.clone()),
            text_param(":filename", upload.filename.clone()),
            text_param(":content_type", upload.content_type.clone()),
            integer_param(":size", upload.size),
            text_param(":storage_key", upload.storage_key.clone()),
            integer_param(":created_at", upload.created_at),
            integer_param(":expires_at", upload.expires_at),
        ])
        .await
        .context(DbStatementSnafu)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, dir::NewDir, test::TestCtx};
    use memo::dir::DirType;
    use memo::file::{FileDto, FileStatus, FileType};
    use memo::job::NewJobDto;
    use std::time::Duration;

    #[tokio::test]
    async fn reserve_rejects_duplicate_storage_keys() -> Result<()> {
        let ctx = TestCtx::new("reserve_upload").await?;
        let mut upload = UploadDto {
            id: "upload-first".into(),
            org_id: "org".into(),
            dir_id: "dir".into(),
            original_filename: "photo.jpg".into(),
            filename: "unique-photo.jpg".into(),
            content_type: "image/jpeg".into(),
            size: 10,
            storage_key: "org/photos/dir/orig/unique-photo.jpg".into(),
            status: UploadStatus::Pending,
            created_at: 100,
            expires_at: 3700,
        };
        ctx.db.uploads.reserve(&upload).await?;
        upload.id = "upload-second".into();
        assert!(matches!(
            ctx.db.uploads.reserve(&upload).await,
            Err(Error::DbStatement { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn finalization_rolls_back_on_job_failure_and_consumes_only_once() -> Result<()> {
        let ctx = TestCtx::new("finalize_upload").await?;
        let dir = ctx
            .db
            .dirs
            .create(
                "org",
                &DirType::Photos,
                &NewDir {
                    name: "album".into(),
                    label: "Album".into(),
                },
            )
            .await?;
        let now = chrono::Utc::now().timestamp();
        let upload = UploadDto {
            id: "upload-first".into(),
            org_id: dir.org_id,
            dir_id: dir.id,
            original_filename: "photo.jpg".into(),
            filename: "stored-photo.jpg".into(),
            content_type: "image/jpeg".into(),
            size: 10,
            storage_key: "org/photos/album/orig/stored-photo.jpg".into(),
            status: UploadStatus::Pending,
            created_at: now,
            expires_at: now + 3600,
        };
        let file = FileDto {
            id: "file-first".into(),
            org_id: upload.org_id.clone(),
            dir_id: upload.dir_id.clone(),
            file_type: FileType::Image,
            status: FileStatus::Processing,
            name: upload.original_filename.clone(),
            filename: upload.filename.clone(),
            content_type: upload.content_type.clone(),
            size: upload.size,
            url: None,
            img_versions: None,
            img_taken_at: None,
            created_at: now,
            updated_at: now,
        };
        let mut job = NewJobDto {
            job_type: "create_image_versions".into(),
            payload: "invalid JSON".into(),
            max_attempts: 3,
            available_at: now,
        };
        ctx.db.uploads.reserve(&upload).await?;
        assert!(matches!(
            ctx.db
                .files
                .finalize_upload(&upload.id, &file, Some(job.clone()))
                .await,
            Err(Error::Validation { .. })
        ));
        let reservation = ctx.db.uploads.get(&upload.id).await?.unwrap();
        assert_eq!(reservation.storage_key, upload.storage_key);
        assert_eq!(reservation.status, UploadStatus::Pending);
        assert!(ctx.db.files.get(&file.id).await?.is_none());

        job.payload = "{}".into();
        let (first, second) = tokio::join!(
            ctx.db
                .files
                .finalize_upload(&upload.id, &file, Some(job.clone())),
            ctx.db.files.finalize_upload(&upload.id, &file, Some(job)),
        );
        assert_ne!(first?.is_some(), second?.is_some());
        assert!(ctx.db.uploads.get(&upload.id).await?.is_none());
        assert!(ctx.db.files.get(&file.id).await?.is_some());
        assert!(
            ctx.db
                .jobs
                .claim_next(Duration::from_secs(30))
                .await?
                .is_some()
        );
        assert!(
            ctx.db
                .jobs
                .claim_next(Duration::from_secs(30))
                .await?
                .is_none()
        );
        Ok(())
    }
}
