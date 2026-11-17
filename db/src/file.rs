use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};
use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use turso::Row;
use validator::Validate;

use crate::db_pool::DbPool;
use crate::error::{DbPrepareSnafu, DbStatementSnafu, DbTransactionSnafu, ValidationSnafu};
use crate::turso_decode::{
    FromTursoRow, collect_count, collect_row, collect_rows, opt_row_integer, opt_row_text,
    row_integer, row_text,
};
use crate::turso_params::{
    integer_param, new_query_params, opt_integer_param, opt_text_param, text_param,
};
use crate::{Error, Result};
use memo::dir::DirDto;
use memo::file::{FileDto, FileType, ImgVersionDto};
use memo::note::NoteDto;
use memo::pagination::Paginated;
use memo::utils::{IdPrefix, generate_prefixed_id};
use memo::validators::flatten_errors;

#[derive(Debug, Clone, Serialize)]
pub struct FileRow {
    pub id: String,
    pub org_id: String,
    pub dir_id: String,
    pub file_type: String,
    pub name: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub is_image: i32,
    pub img_versions: Option<String>,
    pub img_taken_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct FilePayload {
    pub upload_dir: PathBuf,
    pub name: String,
    pub filename: String,
    pub path: PathBuf,
    pub size: i64,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ListFilesParams {
    #[validate(range(min = 1, max = 1000))]
    pub page: Option<i32>,

    #[validate(range(min = 1, max = 50))]
    pub per_page: Option<i32>,

    #[validate(length(min = 0, max = 50))]
    pub keyword: Option<String>,
}

impl From<FileDto> for FileRow {
    fn from(file: FileDto) -> Self {
        let img_versions = match &file.img_versions {
            Some(versions) => {
                let versions_str: String = versions
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(",");

                Some(versions_str)
            }
            None => None,
        };

        let is_image = if file.is_image() { 1 } else { 0 };

        Self {
            id: file.id,
            org_id: file.org_id,
            dir_id: file.dir_id,
            file_type: file.file_type.to_string(),
            name: file.name,
            filename: file.filename,
            content_type: file.content_type,
            size: file.size,
            is_image,
            img_versions,
            img_taken_at: file.img_taken_at,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}

impl From<FileRow> for FileDto {
    fn from(file: FileRow) -> Self {
        let img_versions = match file.img_versions {
            Some(versions_str) => {
                let versions: Vec<ImgVersionDto> = versions_str
                    .split(',')
                    .filter_map(|s| s.parse::<ImgVersionDto>().ok())
                    .collect();

                if !versions.is_empty() {
                    Some(versions)
                } else {
                    None
                }
            }
            None => None,
        };

        // NOTE: This should return an error but current design prevents us to
        // for now, will default to FileType::File on error
        let file_type = FileType::try_from(file.file_type.as_str()).unwrap_or(FileType::File);

        Self {
            id: file.id,
            org_id: file.org_id,
            dir_id: file.dir_id,
            file_type,
            name: file.name,
            filename: file.filename,
            content_type: file.content_type,
            size: file.size,
            img_versions,
            img_taken_at: file.img_taken_at,
            url: None,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}

impl FromTursoRow for FileDto {
    fn from_row(row: &Row) -> Result<Self> {
        let img_versions = match opt_row_text(row, 9)? {
            Some(versions_str) => {
                let versions: Vec<ImgVersionDto> = versions_str
                    .split(',')
                    .filter_map(|s| s.parse::<ImgVersionDto>().ok())
                    .collect();
                if versions.is_empty() {
                    None
                } else {
                    Some(versions)
                }
            }
            None => None,
        };

        // NOTE: This should return an error but current design prevents us to
        // for now, will default to FileType::File on error
        let file_type = FileType::try_from(row_text(row, 3)?.as_str()).unwrap_or(FileType::File);

        Ok(Self {
            id: row_text(row, 0)?,
            org_id: row_text(row, 1)?,
            dir_id: row_text(row, 2)?,
            file_type,
            name: row_text(row, 4)?,
            filename: row_text(row, 5)?,
            content_type: row_text(row, 6)?,
            size: row_integer(row, 7)?,
            img_versions,
            img_taken_at: opt_row_integer(row, 10)?,
            url: None,
            created_at: row_integer(row, 11)?,
            updated_at: row_integer(row, 12)?,
        })
    }
}

pub const MAX_PER_PAGE: i32 = 50;
pub const MAX_FILES: i32 = 1000;

const LATEST_REVISION: &str = "latest";

struct FileIdDto {
    id: String,
}

impl FromTursoRow for FileIdDto {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row_text(row, 0)?,
        })
    }
}

pub struct FileRepo {
    db_pool: Arc<DbPool>,
}

impl FileRepo {
    pub fn new(db_pool: Arc<DbPool>) -> Self {
        Self { db_pool }
    }

    pub async fn list_file_ids(&self) -> Result<Vec<String>> {
        let query =
            "SELECT id FROM files WHERE deleted_at IS NULL ORDER BY created_at ASC".to_string();

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query({}).await.context(DbStatementSnafu)?;
        let items: Vec<FileIdDto> = collect_rows(&mut rows).await?;

        Ok(items.into_iter().map(|item| item.id).collect())
    }

    pub async fn listing_count(&self, dir_id: &str, params: &ListFilesParams) -> Result<i64> {
        let mut query =
            "SELECT COUNT(*) AS total_count FROM files WHERE dir_id = :dir_id".to_string();
        let mut q_params = new_query_params();
        q_params.push(text_param(":dir_id", dir_id.to_owned()));

        if let Some(keyword) = &params.keyword
            && !keyword.is_empty()
        {
            query.push_str(" AND name LIKE :keyword");
            q_params.push(text_param(":keyword", format!("%{}%", keyword)));
        }

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        collect_count(row_result)
    }

    pub async fn list(&self, dir: &DirDto, params: &ListFilesParams) -> Result<Paginated<FileDto>> {
        let errors = params.validate();
        ensure!(
            errors.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&errors.unwrap_err()),
            }
        );

        let total_records = self.listing_count(&dir.id, params).await?;
        let mut page: i32 = 1;
        let mut per_page: i32 = MAX_PER_PAGE;
        let mut offset: i64 = 0;

        if let Some(per_page_param) = params.per_page
            && per_page_param > 0
            && per_page_param <= MAX_PER_PAGE
        {
            per_page = per_page_param;
        }

        let total_pages: i64 = (total_records as f64 / per_page as f64).ceil() as i64;

        if let Some(p) = params.page {
            let p64 = p as i64;
            if p64 > 0 && p64 <= total_pages {
                page = p;
                offset = (p64 - 1) * per_page as i64;
            }
        }

        if total_pages == 0 {
            return Ok(Paginated::new(Vec::new(), page, per_page, total_records));
        }

        let mut query = r#"
            SELECT
                id,
                org_id,
                dir_id,
                file_type,
                name,
                filename,
                content_type,
                size,
                is_image,
                img_versions,
                img_taken_at,
                created_at,
                updated_at
            FROM files
            WHERE dir_id = :dir_id
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":dir_id", dir.id.clone()));

        if let Some(keyword) = &params.keyword
            && !keyword.is_empty()
        {
            query.push_str(" AND name LIKE :keyword");
            q_params.push(text_param(":keyword", format!("%{}%", keyword)));
        }

        query.push_str(" ORDER BY created_at DESC LIMIT :per_page OFFSET :offset");
        q_params.push(integer_param(":per_page", per_page as i64));
        q_params.push(integer_param(":offset", offset));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query(q_params).await.context(DbStatementSnafu)?;
        let items: Vec<FileDto> = collect_rows(&mut rows).await?;

        Ok(Paginated::new(items, page, per_page, total_records))
    }

    pub async fn create(&self, data: FileDto) -> Result<FileDto> {
        let file: FileRow = data.clone().into();

        let query = r#"
            INSERT INTO files
            (
                id,
                org_id,
                dir_id,
                file_type,
                name,
                filename,
                content_type,
                size,
                is_image,
                img_versions,
                img_taken_at,
                created_at,
                updated_at
            )
            VALUES
            (
                :id,
                :org_id,
                :dir_id,
                :file_type,
                :name,
                :filename,
                :content_type,
                :size,
                :is_image,
                :img_versions,
                :img_taken_at,
                :created_at,
                :updated_at
            )
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", file.id.clone()));
        q_params.push(text_param(":org_id", file.org_id.clone()));
        q_params.push(text_param(":dir_id", file.dir_id.clone()));
        q_params.push(text_param(":file_type", file.file_type.to_string()));
        q_params.push(text_param(":name", file.name.clone()));
        q_params.push(text_param(":filename", file.filename.clone()));
        q_params.push(text_param(":content_type", file.content_type.clone()));
        q_params.push(integer_param(":size", file.size));
        q_params.push(integer_param(":is_image", file.is_image as i64));
        q_params.push(opt_text_param(":img_versions", file.img_versions.clone()));
        q_params.push(opt_integer_param(":img_taken_at", file.img_taken_at));
        q_params.push(integer_param(":created_at", file.created_at));
        q_params.push(integer_param(":updated_at", file.updated_at));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(file.into())
    }

    pub async fn retry_create(&self, file_dto: FileDto, max_retries: usize) -> Result<FileDto> {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(2);

        loop {
            match self.create(file_dto.clone()).await {
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

    pub async fn retry_create_note(
        &self,
        file: FileDto,
        cipher_content: String,
        checksum: String,
        max_retries: usize,
    ) -> Result<FileDto> {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(2);

        loop {
            match self
                .create_note(file.clone(), cipher_content.clone(), checksum.clone())
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
                    }
                    _ => return Err(Error::DbStatement { source }),
                },
                Err(Error::DbTransaction { source }) => match source {
                    turso::Error::Busy(..) => {
                        attempts += 1;
                        if attempts >= max_retries {
                            return Err(Error::DbTransaction { source });
                        }

                        sleep(delay).await;
                        delay = min(delay.saturating_mul(2), max_delay);
                    }
                    _ => return Err(Error::DbTransaction { source }),
                },
                Err(error) => return Err(error),
            }
        }
    }

    async fn create_note(
        &self,
        file: FileDto,
        cipher_content: String,
        checksum: String,
    ) -> Result<FileDto> {
        let note = NoteDto {
            id: generate_prefixed_id(IdPrefix::Note),
            file_id: file.id.clone(),
            content: cipher_content,
            next_revision: LATEST_REVISION.to_string(),
            checksum,
            created_at: chrono::Utc::now().timestamp(),
        };

        let file_row: FileRow = file.clone().into();
        let file_query = r#"
            INSERT INTO files
            (
                id, org_id, dir_id, file_type, name, filename, content_type, size,
                is_image, img_versions, img_taken_at, created_at, updated_at
            )
            VALUES
            (
                :id, :org_id, :dir_id, :file_type, :name, :filename, :content_type, :size,
                :is_image, :img_versions, :img_taken_at, :created_at, :updated_at
            )
        "#;
        let note_query = r#"
            INSERT INTO notes (id, file_id, content, next_revision, checksum, created_at)
            VALUES (:id, :file_id, :content, :next_revision, :checksum, :created_at)
        "#;

        let mut file_params = new_query_params();
        file_params.push(text_param(":id", file_row.id));
        file_params.push(text_param(":org_id", file_row.org_id));
        file_params.push(text_param(":dir_id", file_row.dir_id));
        file_params.push(text_param(":file_type", file_row.file_type));
        file_params.push(text_param(":name", file_row.name));
        file_params.push(text_param(":filename", file_row.filename));
        file_params.push(text_param(":content_type", file_row.content_type));
        file_params.push(integer_param(":size", file_row.size));
        file_params.push(integer_param(":is_image", file_row.is_image as i64));
        file_params.push(opt_text_param(":img_versions", file_row.img_versions));
        file_params.push(opt_integer_param(":img_taken_at", file_row.img_taken_at));
        file_params.push(integer_param(":created_at", file_row.created_at));
        file_params.push(integer_param(":updated_at", file_row.updated_at));

        let mut note_params = new_query_params();
        note_params.push(text_param(":id", note.id));
        note_params.push(text_param(":file_id", note.file_id));
        note_params.push(text_param(":content", note.content));
        note_params.push(text_param(":next_revision", note.next_revision));
        note_params.push(text_param(":checksum", note.checksum));
        note_params.push(integer_param(":created_at", note.created_at));

        let conn = self.db_pool.acquire().await?;
        conn.execute("BEGIN CONCURRENT", ())
            .await
            .context(DbTransactionSnafu)?;

        let operation: Result<()> = async {
            let mut file_stmt = conn.prepare(file_query).await.context(DbPrepareSnafu)?;
            file_stmt
                .execute(file_params)
                .await
                .context(DbStatementSnafu)?;

            let mut note_stmt = conn.prepare(note_query).await.context(DbPrepareSnafu)?;
            note_stmt
                .execute(note_params)
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

        Ok(file)
    }

    pub async fn update_note(
        &self,
        file_id: String,
        cipher_content: String,
        checksum: String,
        size: i64,
        updated_at: i64,
    ) -> Result<NoteDto> {
        let note = NoteDto {
            id: generate_prefixed_id(IdPrefix::Note),
            file_id: file_id.clone(),
            content: cipher_content,
            next_revision: LATEST_REVISION.to_string(),
            checksum,
            created_at: chrono::Utc::now().timestamp(),
        };

        let update_revision_query = r#"
            UPDATE notes
            SET next_revision = :new_revision_id
            WHERE file_id = :file_id AND next_revision = :latest_revision
        "#;
        let insert_revision_query = r#"
            INSERT INTO notes (id, file_id, content, next_revision, checksum, created_at)
            VALUES (:id, :file_id, :content, :next_revision, :checksum, :created_at)
        "#;
        let update_file_query = r#"
            UPDATE files
            SET size = :size, updated_at = :updated_at
            WHERE id = :id
        "#;

        let mut update_revision_params = new_query_params();
        update_revision_params.push(text_param(":new_revision_id", note.id.clone()));
        update_revision_params.push(text_param(":file_id", file_id.clone()));
        update_revision_params.push(text_param(":latest_revision", LATEST_REVISION.to_string()));

        let mut insert_revision_params = new_query_params();
        insert_revision_params.push(text_param(":id", note.id.clone()));
        insert_revision_params.push(text_param(":file_id", note.file_id.clone()));
        insert_revision_params.push(text_param(":content", note.content.clone()));
        insert_revision_params.push(text_param(":next_revision", note.next_revision.clone()));
        insert_revision_params.push(text_param(":checksum", note.checksum.clone()));
        insert_revision_params.push(integer_param(":created_at", note.created_at));

        let mut update_file_params = new_query_params();
        update_file_params.push(integer_param(":size", size));
        update_file_params.push(integer_param(":updated_at", updated_at));
        update_file_params.push(text_param(":id", file_id));

        let conn = self.db_pool.acquire().await?;
        conn.execute("BEGIN CONCURRENT", ())
            .await
            .context(DbTransactionSnafu)?;

        let operation: Result<()> = async {
            let mut update_revision_stmt = conn
                .prepare(update_revision_query)
                .await
                .context(DbPrepareSnafu)?;
            update_revision_stmt
                .execute(update_revision_params)
                .await
                .context(DbStatementSnafu)?;

            let mut insert_revision_stmt = conn
                .prepare(insert_revision_query)
                .await
                .context(DbPrepareSnafu)?;
            insert_revision_stmt
                .execute(insert_revision_params)
                .await
                .context(DbStatementSnafu)?;

            let mut update_file_stmt = conn
                .prepare(update_file_query)
                .await
                .context(DbPrepareSnafu)?;
            update_file_stmt
                .execute(update_file_params)
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

    pub async fn get(&self, id: &str) -> Result<Option<FileDto>> {
        let query = r#"
            SELECT
                id,
                org_id,
                dir_id,
                file_type,
                name,
                filename,
                content_type,
                size,
                is_image,
                img_versions,
                img_taken_at,
                created_at,
                updated_at
            FROM files
            WHERE id = :id
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<FileDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn find_by_name(&self, dir_id: &str, name: &str) -> Result<Option<FileDto>> {
        let query = r#"
            SELECT
                id,
                org_id,
                dir_id,
                file_type,
                name,
                filename,
                content_type,
                size,
                is_image,
                img_versions,
                img_taken_at,
                created_at,
                updated_at
            FROM files
            WHERE dir_id = :dir_id AND name = :name
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":dir_id", dir_id.to_owned()));
        q_params.push(text_param(":name", name.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<FileDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn count_by_dir(&self, dir_id: &str) -> Result<i64> {
        let query = r#"
            SELECT COUNT(*) AS total_count
            FROM files
            WHERE dir_id = :dir_id
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":dir_id", dir_id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        collect_count(row_result)
    }

    pub async fn move_to_dir(&self, old_dir_id: &str, new_dir_id: &str) -> Result<()> {
        let query = r#"
            UPDATE files
            SET dir_id = :new_dir_id
            WHERE dir_id = :old_dir_id
        "#;
        let mut q_params = new_query_params();
        q_params.push(text_param(":new_dir_id", new_dir_id.to_owned()));
        q_params.push(text_param(":old_dir_id", old_dir_id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;
        Ok(())
    }

    pub async fn update_id(&self, old_id: &str, new_id: &str) -> Result<()> {
        let query = r#"
            UPDATE files
            SET id = :new_id
            WHERE id = :old_id
        "#;
        let mut q_params = new_query_params();
        q_params.push(text_param(":new_id", new_id.to_owned()));
        q_params.push(text_param(":old_id", old_id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;
        Ok(())
    }

    pub async fn delete_note(&self, file_id: &str) -> Result<()> {
        let delete_revisions_query = "DELETE FROM notes WHERE file_id = :file_id";
        let delete_file_query = "DELETE FROM files WHERE id = :id";

        let mut revision_params = new_query_params();
        revision_params.push(text_param(":file_id", file_id.to_owned()));

        let mut file_params = new_query_params();
        file_params.push(text_param(":id", file_id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        conn.execute("BEGIN CONCURRENT", ())
            .await
            .context(DbTransactionSnafu)?;

        let operation: Result<()> = async {
            let mut revisions_stmt = conn
                .prepare(delete_revisions_query)
                .await
                .context(DbPrepareSnafu)?;
            revisions_stmt
                .execute(revision_params)
                .await
                .context(DbStatementSnafu)?;

            let mut file_stmt = conn
                .prepare(delete_file_query)
                .await
                .context(DbPrepareSnafu)?;
            file_stmt
                .execute(file_params)
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

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let query = "DELETE FROM files WHERE id = :id".to_string();
        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::TestCtx;

    fn note_file() -> FileDto {
        FileDto {
            id: "fil-note".to_string(),
            org_id: "org-note".to_string(),
            dir_id: "dir-note".to_string(),
            file_type: FileType::Note,
            name: "Shopping list".to_string(),
            filename: "Shopping list".to_string(),
            content_type: "text/plain".to_string(),
            size: 4,
            url: None,
            img_versions: None,
            img_taken_at: None,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[tokio::test]
    async fn retry_create_note_creates_file_and_initial_revision() {
        let ctx = TestCtx::new("create-note").await.unwrap();
        let file = note_file();

        let created = ctx
            .db
            .files
            .retry_create_note(
                file.clone(),
                "ciphertext".to_string(),
                "checksum".to_string(),
                5,
            )
            .await
            .unwrap();

        assert_eq!(created.id, file.id);
        assert_eq!(
            ctx.db.files.get(&file.id).await.unwrap().unwrap().name,
            file.name
        );

        let note = ctx.db.notes.get_note(&file.id).await.unwrap().unwrap();
        assert_eq!(note.file_id, file.id);
        assert_eq!(note.content, "ciphertext");
        assert_eq!(note.checksum, "checksum");
        assert_eq!(note.next_revision, LATEST_REVISION);
    }

    #[tokio::test]
    async fn update_note_creates_revision_and_updates_file_metadata() {
        let ctx = TestCtx::new("update-note").await.unwrap();
        let file = note_file();

        ctx.db
            .files
            .retry_create_note(
                file.clone(),
                "ciphertext".to_string(),
                "checksum".to_string(),
                5,
            )
            .await
            .unwrap();
        let original = ctx.db.notes.get_note(&file.id).await.unwrap().unwrap();

        let updated = ctx
            .db
            .files
            .update_note(
                file.id.clone(),
                "updated-ciphertext".to_string(),
                "updated-checksum".to_string(),
                18,
                2,
            )
            .await
            .unwrap();

        assert_ne!(updated.id, original.id);
        let latest = ctx.db.notes.get_note(&file.id).await.unwrap().unwrap();
        assert_eq!(latest.id, updated.id);
        assert_eq!(latest.content, "updated-ciphertext");
        assert_eq!(latest.checksum, "updated-checksum");

        let updated_file = ctx.db.files.get(&file.id).await.unwrap().unwrap();
        assert_eq!(updated_file.size, 18);
        assert_eq!(updated_file.updated_at, 2);
    }

    #[tokio::test]
    async fn delete_note_removes_file_and_revisions() {
        let ctx = TestCtx::new("delete-note").await.unwrap();
        let file = note_file();

        ctx.db
            .files
            .retry_create_note(
                file.clone(),
                "ciphertext".to_string(),
                "checksum".to_string(),
                5,
            )
            .await
            .unwrap();
        ctx.db
            .files
            .update_note(
                file.id.clone(),
                "updated-ciphertext".to_string(),
                "updated-checksum".to_string(),
                18,
                2,
            )
            .await
            .unwrap();

        ctx.db.files.delete_note(&file.id).await.unwrap();

        assert!(ctx.db.files.get(&file.id).await.unwrap().is_none());
        assert!(ctx.db.notes.get_note(&file.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_revisions_returns_newest_first_and_caps_at_fifty() {
        let ctx = TestCtx::new("list-revisions").await.unwrap();
        let file = note_file();

        ctx.db
            .files
            .retry_create_note(
                file.clone(),
                "ciphertext".to_string(),
                "checksum".to_string(),
                5,
            )
            .await
            .unwrap();
        let initial = ctx.db.notes.get_note(&file.id).await.unwrap().unwrap();

        let mut latest_id = String::new();
        for revision in 1..=50 {
            let updated = ctx
                .db
                .files
                .update_note(
                    file.id.clone(),
                    format!("ciphertext-{revision}"),
                    format!("checksum-{revision}"),
                    revision,
                    revision,
                )
                .await
                .unwrap();
            latest_id = updated.id;
        }

        let revisions = ctx.db.notes.list_revisions(&file.id).await.unwrap();
        assert_eq!(revisions.len(), 50);
        assert_eq!(revisions.first().unwrap().id, latest_id);
        assert!(!revisions.iter().any(|revision| revision.id == initial.id));
        assert!(revisions.windows(2).all(|pair| {
            (pair[0].created_at, &pair[0].id) >= (pair[1].created_at, &pair[1].id)
        }));

        assert!(
            ctx.db
                .notes
                .get_revision(&file.id, &latest_id)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            ctx.db
                .notes
                .get_revision(&file.id, &generate_prefixed_id(IdPrefix::Note))
                .await
                .unwrap()
                .is_none()
        );
    }
}
