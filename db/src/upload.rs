use std::sync::Arc;

use snafu::{ResultExt, ensure};

use crate::Result;
use crate::db_pool::DbPool;
use crate::error::{DbPrepareSnafu, DbStatementSnafu, ValidationSnafu};
use crate::turso_params::{integer_param, text_param};
use memo::file::{UploadDto, UploadStatus};

pub struct UploadRepo {
    db_pool: Arc<DbPool>,
}

impl UploadRepo {
    pub(crate) fn new(db_pool: Arc<DbPool>) -> Self {
        Self { db_pool }
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
    use crate::{Error, test::TestCtx};

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
}
