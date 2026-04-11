use memo::dir::DirDto;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};
use std::path::PathBuf;
use turso::{Connection, Row};
use validator::Validate;

use crate::Result;
use crate::error::{DbPrepareSnafu, DbStatementSnafu, ValidationSnafu};
use crate::turso_decode::{
    FromTursoRow, collect_count, collect_row, collect_rows, opt_row_integer, opt_row_text,
    row_integer, row_text,
};
use crate::turso_params::{
    integer_param, new_query_params, opt_integer_param, opt_text_param, text_param,
};
use memo::file::{FileDto, ImgVersionDto};
use memo::pagination::Paginated;
use memo::validators::flatten_errors;

#[derive(Debug, Clone, Serialize)]
pub struct FileObject {
    pub id: String,
    pub dir_id: String,
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

impl From<FileDto> for FileObject {
    fn from(file: FileDto) -> Self {
        let img_versions = match file.img_versions {
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

        Self {
            id: file.id,
            dir_id: file.dir_id,
            name: file.name,
            filename: file.filename,
            content_type: file.content_type,
            size: file.size,
            is_image: if file.is_image { 1 } else { 0 },
            img_versions,
            img_taken_at: file.img_taken_at,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}

impl From<FileObject> for FileDto {
    fn from(file: FileObject) -> Self {
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

        Self {
            id: file.id,
            dir_id: file.dir_id,
            name: file.name,
            filename: file.filename,
            content_type: file.content_type,
            size: file.size,
            is_image: file.is_image == 1,
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
        let img_versions = match opt_row_text(row, 7)? {
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

        Ok(Self {
            id: row_text(row, 0)?,
            dir_id: row_text(row, 1)?,
            name: row_text(row, 2)?,
            filename: row_text(row, 3)?,
            content_type: row_text(row, 4)?,
            size: row_integer(row, 5)?,
            is_image: matches!(row_integer(row, 6)?, 1),
            img_versions,
            img_taken_at: opt_row_integer(row, 8)?,
            url: None,
            created_at: row_integer(row, 9)?,
            updated_at: row_integer(row, 10)?,
        })
    }
}

pub const MAX_PER_PAGE: i32 = 50;
pub const MAX_FILES: i32 = 1000;

pub struct FileRepo {
    db_pool: Connection,
}

impl FileRepo {
    pub fn new(db_pool: Connection) -> Self {
        Self { db_pool }
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

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
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
                dir_id,
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

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query(q_params).await.context(DbStatementSnafu)?;
        let items: Vec<FileDto> = collect_rows(&mut rows).await?;

        Ok(Paginated::new(items, page, per_page, total_records))
    }

    pub async fn create(&self, file_dto: FileDto) -> Result<FileDto> {
        let file: FileObject = file_dto.clone().into();

        let query = r#"
            INSERT INTO files
            (
                id,
                dir_id,
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
                :dir_id,
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
        q_params.push(text_param(":dir_id", file.dir_id.clone()));
        q_params.push(text_param(":name", file.name.clone()));
        q_params.push(text_param(":filename", file.filename.clone()));
        q_params.push(text_param(":content_type", file.content_type.clone()));
        q_params.push(integer_param(":size", file.size));
        q_params.push(integer_param(":is_image", file.is_image as i64));
        q_params.push(opt_text_param(":img_versions", file.img_versions.clone()));
        q_params.push(opt_integer_param(":img_taken_at", file.img_taken_at));
        q_params.push(integer_param(":created_at", file.created_at));
        q_params.push(integer_param(":updated_at", file.updated_at));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(file.into())
    }

    pub async fn get(&self, id: &str) -> Result<Option<FileDto>> {
        let query = r#"
            SELECT
                id,
                dir_id,
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

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<FileDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn find_by_name(&self, dir_id: &str, name: &str) -> Result<Option<FileDto>> {
        let query = r#"
            SELECT
                id,
                dir_id,
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

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
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

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        collect_count(row_result)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let query = "DELETE FROM files WHERE id = :id".to_string();
        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(())
    }
}
