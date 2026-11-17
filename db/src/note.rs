use std::cmp::min;
use std::sync::Arc;
use std::time::Duration;

use snafu::ResultExt;
use tokio::time::sleep;
use turso::Row;

use crate::db_pool::DbPool;
use crate::error::{DbPrepareSnafu, DbStatementSnafu, DbTransactionSnafu};
use crate::turso_decode::{FromTursoRow, collect_row, collect_rows, row_integer, row_text};
use crate::turso_params::{integer_param, new_query_params, text_param};
use crate::{Error, Result};
use memo::note::NoteDto;
use memo::utils::{IdPrefix, generate_prefixed_id};

const LATEST_REVISION: &'static str = "latest";

impl FromTursoRow for NoteDto {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row_text(row, 0)?,
            file_id: row_text(row, 1)?,
            content: row_text(row, 2)?,
            next_revision: row_text(row, 3)?,
            checksum: row_text(row, 4)?,
            created_at: row_integer(row, 5)?,
        })
    }
}

pub struct NoteRepo {
    db_pool: Arc<DbPool>,
}

impl NoteRepo {
    pub fn new(db_pool: Arc<DbPool>) -> Self {
        Self { db_pool }
    }

    pub async fn create_revision(
        &self,
        file_id: String,
        cipher_content: String,
        checksum: String,
    ) -> Result<NoteDto> {
        let note = NoteDto {
            id: generate_prefixed_id(IdPrefix::Note),
            file_id,
            content: cipher_content,
            next_revision: LATEST_REVISION.to_string(),
            checksum: checksum.clone(),
            created_at: chrono::Utc::now().timestamp(),
        };

        let update_query = r#"
            UPDATE notes
            SET next_revision = :new_revision_id
            WHERE file_id = :file_id AND next_revision = :latest_revision
        "#;

        let insert_query = r#"
            INSERT INTO notes
            (
                id,
                file_id,
                content,
                next_revision,
                checksum,
                created_at
            )
            VALUES
            (
                :id,
                :file_id,
                :content,
                :next_revision,
                :checksum,
                :created_at
            )
        "#;

        let mut update_params = new_query_params();
        update_params.push(text_param(":new_revision_id", note.id.clone()));
        update_params.push(text_param(":file_id", note.file_id.clone()));
        update_params.push(text_param(":latest_revision", LATEST_REVISION.to_string()));

        let mut insert_params = new_query_params();
        insert_params.push(text_param(":id", note.id.clone()));
        insert_params.push(text_param(":file_id", note.file_id.clone()));
        insert_params.push(text_param(":content", note.content.clone()));
        insert_params.push(text_param(":next_revision", LATEST_REVISION.to_string()));
        insert_params.push(text_param(":checksum", checksum));
        insert_params.push(integer_param(":created_at", note.created_at));

        let conn = self.db_pool.acquire().await?;

        conn.execute("BEGIN CONCURRENT", ())
            .await
            .context(DbTransactionSnafu)?;

        let operation: Result<()> = async {
            let mut update_stmt = conn.prepare(update_query).await.context(DbPrepareSnafu)?;
            update_stmt
                .execute(update_params)
                .await
                .context(DbStatementSnafu)?;

            let mut insert_stmt = conn.prepare(insert_query).await.context(DbPrepareSnafu)?;
            insert_stmt
                .execute(insert_params)
                .await
                .context(DbStatementSnafu)?;

            Ok(())
        }
        .await;

        if let Err(error) = operation {
            conn.execute("ROLLBACK", ())
                .await
                .context(DbTransactionSnafu)?;
            return Err(error);
        }

        if let Err(error) = conn.execute("COMMIT", ()).await.context(DbTransactionSnafu) {
            conn.execute("ROLLBACK", ())
                .await
                .context(DbTransactionSnafu)?;
            return Err(error);
        }

        Ok(note)
    }

    pub async fn retry_create_revision(
        &self,
        file_id: String,
        cipher_content: String,
        checksum: String,
        max_retries: usize,
    ) -> Result<NoteDto> {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(2);

        loop {
            match self
                .create_revision(file_id.clone(), cipher_content.clone(), checksum.clone())
                .await
            {
                Ok(result) => return Ok(result),
                Err(Error::DbStatement { source }) => match source {
                    turso::Error::Busy(..) => {
                        attempts += 1;
                        if attempts >= max_retries {
                            return Err(Error::DbStatement { source });
                        }

                        sleep(delay).await;
                        delay = min(delay.saturating_mul(2), max_delay);
                        // Retries...
                    }
                    _ => {
                        return Err(Error::DbStatement { source });
                    }
                },
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    /// Get the latest note revision for a given file_id
    pub async fn get_note(&self, file_id: &str) -> Result<Option<NoteDto>> {
        let query = r#"
            SELECT
                id,
                file_id,
                content,
                next_revision,
                checksum,
                created_at
            FROM notes
            WHERE file_id = :file_id AND next_revision = :next_revision
            ORDER BY id DESC
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":file_id", file_id.to_owned()));
        q_params.push(text_param(":next_revision", LATEST_REVISION.to_string()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<NoteDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn get_revision(&self, file_id: &str, revision_id: &str) -> Result<Option<NoteDto>> {
        let query = r#"
            SELECT
                id,
                file_id,
                content,
                next_revision,
                checksum,
                created_at
            FROM notes
            WHERE file_id = :file_id AND id = :revision_id
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":file_id", file_id.to_owned()));
        q_params.push(text_param(":revision_id", revision_id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<NoteDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn list_revisions(&self, file_id: &str) -> Result<Vec<NoteDto>> {
        let query = r#"
            SELECT
                id,
                file_id,
                content,
                next_revision,
                checksum,
                created_at
            FROM notes
            WHERE file_id = :file_id
            ORDER BY created_at DESC, id DESC
            LIMIT 50
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":file_id", file_id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query(q_params).await.context(DbStatementSnafu)?;
        collect_rows(&mut rows).await
    }

    pub async fn delete_revisions(&self, file_id: &str) -> Result<()> {
        let query = r#"
            DELETE FROM notes
            WHERE file_id = :file_id
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":file_id", file_id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(())
    }
}
