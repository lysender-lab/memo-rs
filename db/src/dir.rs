use serde::Deserialize;
use snafu::{ResultExt, ensure};
use std::cmp::min;
use std::time::Duration;
use tokio::time::sleep;
use turso::Row;
use validator::Validate;

use crate::error::{DbPrepareSnafu, DbStatementSnafu, ValidationSnafu};
use crate::turso_decode::{
    FromTursoRow, collect_count, collect_row, collect_rows, row_dir_type, row_integer, row_text,
};
use crate::turso_params::{integer_param, new_query_params, opt_integer_param, text_param};
use crate::{Error, Result};
use memo::dir::{DirDto, DirType};
use memo::pagination::Paginated;
use memo::utils::{IdPrefix, generate_prefixed_id};
use memo::validators::flatten_errors;
use turso::Connection;

impl FromTursoRow for DirDto {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row_text(row, 0)?,
            org_id: row_text(row, 1)?,
            dir_type: row_dir_type(row, 2)?,
            name: row_text(row, 3)?,
            label: row_text(row, 4)?,
            created_at: row_integer(row, 5)?,
            updated_at: row_integer(row, 6)?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct NewDir {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::sluggable"))]
    pub name: String,

    #[validate(length(min = 1, max = 60))]
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateDir {
    #[validate(length(min = 1, max = 60))]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ListDirsParams {
    #[validate(range(min = 1, max = 1000))]
    pub page: Option<i32>,

    #[validate(range(min = 1, max = 50))]
    pub per_page: Option<i32>,

    #[validate(length(min = 0, max = 50))]
    pub keyword: Option<String>,
}

pub const MAX_DIRS: i32 = 1000;
pub const MAX_PER_PAGE: i32 = 50;

pub struct DirRepo {
    db_pool: Connection,
}

impl DirRepo {
    pub fn new(db_pool: Connection) -> Self {
        Self { db_pool }
    }

    async fn listing_count(
        &self,
        org_id: &str,
        dir_type: &DirType,
        params: &ListDirsParams,
    ) -> Result<i64> {
        let mut query = r#"
            SELECT
                COUNT(*) AS total_count
            FROM
                dirs
            WHERE
                org_id = :org_id
                AND dir_type = :dir_type
                AND deleted_at IS NULL
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":org_id", org_id.to_owned()));
        q_params.push(text_param(":dir_type", dir_type.to_string()));

        if let Some(keyword) = &params.keyword
            && !keyword.is_empty()
        {
            query.push_str(" AND (name LIKE :keyword OR label LIKE :keyword)");
            q_params.push(text_param(":keyword", format!("%{}%", keyword)));
        }

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        collect_count(row_result)
    }

    pub async fn list(
        &self,
        org_id: &str,
        dir_type: &DirType,
        params: &ListDirsParams,
    ) -> Result<Paginated<DirDto>> {
        let valid_res = params.validate();
        ensure!(
            valid_res.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&valid_res.unwrap_err()),
            }
        );

        let total_records = self.listing_count(org_id, dir_type, params).await?;
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
                dir_type,
                name,
                label,
                created_at,
                updated_at
            FROM dirs
            WHERE
                org_id = :org_id
                AND dir_type = :dir_type
                AND deleted_at IS NULL
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":org_id", org_id.to_owned()));
        q_params.push(text_param(":dir_type", dir_type.to_string()));

        if let Some(keyword) = &params.keyword
            && !keyword.is_empty()
        {
            query.push_str(" AND (name LIKE :keyword OR label LIKE :keyword)");
            q_params.push(text_param(":keyword", format!("%{}%", keyword)));
        }

        query.push_str(" ORDER BY updated_at DESC LIMIT :per_page OFFSET :offset");
        q_params.push(integer_param(":per_page", per_page as i64));
        q_params.push(integer_param(":offset", offset));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query(q_params).await.context(DbStatementSnafu)?;
        let items: Vec<DirDto> = collect_rows(&mut rows).await?;

        Ok(Paginated::new(items, page, per_page, total_records))
    }

    pub async fn count(&self, org_id: &str, dir_type: &DirType) -> Result<i64> {
        let query = r#"
            SELECT COUNT(*) AS total_count
            FROM dirs
            WHERE org_id = :org_id AND dir_type = :dir_type AND deleted_at IS NULL
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":org_id", org_id.to_owned()));
        q_params.push(text_param(":dir_type", dir_type.to_string()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        collect_count(row_result)
    }

    pub async fn create_full(&self, data: DirDto) -> Result<()> {
        let query = r#"
            INSERT INTO dirs
            (
                id,
                org_id,
                dir_type,
                name,
                label,
                created_at,
                updated_at,
                deleted_at
            )
            VALUES
            (
                :id,
                :org_id,
                :dir_type,
                :name,
                :label,
                :created_at,
                :updated_at,
                NULL
            )
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", data.id.clone()));
        q_params.push(text_param(":org_id", data.org_id.clone()));
        q_params.push(text_param(":dir_type", data.dir_type.to_string()));
        q_params.push(text_param(":name", data.name.clone()));
        q_params.push(text_param(":label", data.label.clone()));
        q_params.push(integer_param(":created_at", data.created_at));
        q_params.push(integer_param(":updated_at", data.updated_at));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(())
    }

    pub async fn create(&self, org_id: &str, dir_type: &DirType, data: &NewDir) -> Result<DirDto> {
        let today = chrono::Utc::now().timestamp();
        let id = generate_prefixed_id(IdPrefix::Dir);

        let query = r#"
            INSERT INTO dirs
            (
                id,
                org_id,
                dir_type,
                name,
                label,
                created_at,
                updated_at,
                deleted_at
            )
            VALUES
            (
                :id,
                :org_id,
                :dir_type,
                :name,
                :label,
                :created_at,
                :updated_at,
                NULL
            )
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.clone()));
        q_params.push(text_param(":org_id", org_id.to_owned()));
        q_params.push(text_param(":dir_type", dir_type.to_string()));
        q_params.push(text_param(":name", data.name.clone()));
        q_params.push(text_param(":label", data.label.clone()));
        q_params.push(integer_param(":created_at", today));
        q_params.push(integer_param(":updated_at", today));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;

        Ok(DirDto {
            id,
            org_id: org_id.to_owned(),
            dir_type: dir_type.clone(),
            name: data.name.clone(),
            label: data.label.clone(),
            created_at: today,
            updated_at: today,
        })
    }

    pub async fn get(&self, id: &str) -> Result<Option<DirDto>> {
        let query = r#"
            SELECT
                id,
                org_id,
                dir_type,
                name,
                label,
                created_at,
                updated_at
            FROM dirs
            WHERE id = :id AND deleted_at IS NULL
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<DirDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn retry_get(&self, id: &str, max_retries: usize) -> Result<Option<DirDto>> {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(2);

        loop {
            match self.get(id).await {
                Ok(result) => return Ok(result),
                Err(Error::DbResult { source }) => match source {
                    turso::Error::Misuse(..) => {
                        attempts += 1;
                        if attempts >= max_retries {
                            return Err(Error::DbResult { source });
                        }

                        sleep(delay).await;
                        delay = min(delay.saturating_mul(2), max_delay);
                        // Retries...
                    }
                    _ => {
                        return Err(Error::DbResult { source });
                    }
                },
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn find_by_name(
        &self,
        org_id: &str,
        dir_type: &DirType,
        name: &str,
    ) -> Result<Option<DirDto>> {
        let query = r#"
            SELECT
                id,
                org_id,
                dir_type,
                name,
                label,
                created_at,
                updated_at
            FROM
                dirs
            WHERE
                org_id = :org_id
                AND dir_type = :dir_type
                AND name = :name
                AND deleted_at IS NULL
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":org_id", org_id.to_owned()));
        q_params.push(text_param(":dir_type", dir_type.to_string()));
        q_params.push(text_param(":name", name.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<DirDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn update(&self, id: &str, data: &UpdateDir) -> Result<bool> {
        let Some(label) = data.label.clone() else {
            return Ok(false);
        };

        let query = r#"
            UPDATE dirs
            SET label = :label
            WHERE id = :id AND deleted_at IS NULL
        "#;

        let mut q_params = new_query_params();
        q_params.push(text_param(":label", label));
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let affected = stmt.execute(q_params).await.context(DbStatementSnafu)?;
        Ok(affected > 0)
    }

    pub async fn update_timestamp(&self, id: &str, timestamp: i64) -> Result<bool> {
        let query = r#"
            UPDATE dirs
            SET updated_at = :updated_at
            WHERE id = :id AND deleted_at IS NULL
        "#;

        let mut q_params = new_query_params();
        q_params.push(integer_param(":updated_at", timestamp));
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let affected = stmt.execute(q_params).await.context(DbStatementSnafu)?;
        Ok(affected > 0)
    }

    pub async fn retry_update_timestamp(
        &self,
        id: &str,
        timestamp: i64,
        max_retries: usize,
    ) -> Result<bool> {
        // Ideally, this should use a transaction
        // but then again, we are just bumping the timestamp
        // so no big deal if it is less accurate
        let mut attempts = 0;
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(2);

        loop {
            match self.update_timestamp(id, timestamp).await {
                Ok(result) => return Ok(result),
                Err(Error::DbStatement { source }) => match source {
                    turso::Error::Misuse(..) => {
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

    pub async fn delete(&self, id: &str) -> Result<()> {
        let today = chrono::Utc::now().timestamp();
        let query = r#"
            UPDATE dirs
            SET deleted_at = :deleted_at
            WHERE id = :id AND deleted_at IS NULL
        "#;
        let mut q_params = new_query_params();
        q_params.push(opt_integer_param(":deleted_at", Some(today)));
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(q_params).await.context(DbStatementSnafu)?;
        Ok(())
    }
    pub async fn test_read(&self) -> Result<()> {
        let query = r#"
            SELECT
                id,
                org_id,
                dir_type,
                name,
                label,
                created_at,
                updated_at
            FROM dirs
            LIMIT 1
        "#
        .to_string();

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row({}).await;
        let _: Option<DirDto> = collect_row(row_result)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_dir() {
        let data = NewDir {
            name: "hello-world".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_ok());

        let data = NewDir {
            name: "hello_world".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_err());

        let data = NewDir {
            name: "".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_err());
    }
}
