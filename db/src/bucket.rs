use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use turso::{Connection, Row};
use validator::Validate;

use crate::Result;
use crate::error::{DbPrepareSnafu, DbStatementSnafu};
use crate::turso_decode::{
    FromTursoRow, collect_count, collect_row, collect_rows, row_integer, row_text,
};
use crate::turso_params::{integer_param, new_query_params, opt_integer_param, text_param};
use memo::{bucket::BucketDto, utils::generate_id};

#[derive(Debug, Clone, Serialize)]
pub struct Bucket {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub label: String,
    pub images_only: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct NewBucket {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::sluggable"))]
    pub name: String,

    #[validate(length(min = 1, max = 60))]
    pub label: String,

    pub images_only: bool,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateBucket {
    #[validate(length(min = 1, max = 60))]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ListBucketsParams {
    #[validate(range(min = 1, max = 1000))]
    pub page: Option<i32>,

    #[validate(range(min = 1, max = 50))]
    pub per_page: Option<i32>,

    #[validate(length(min = 0, max = 50))]
    pub keyword: Option<String>,
}

impl From<BucketDto> for Bucket {
    fn from(dto: BucketDto) -> Self {
        Bucket {
            id: dto.id,
            client_id: dto.client_id,
            name: dto.name,
            label: dto.label,
            images_only: if dto.images_only { 1 } else { 0 },
            created_at: dto.created_at,
        }
    }
}

impl From<Bucket> for BucketDto {
    fn from(bucket: Bucket) -> Self {
        BucketDto {
            id: bucket.id,
            client_id: bucket.client_id,
            name: bucket.name,
            label: bucket.label,
            images_only: bucket.images_only == 1,
            created_at: bucket.created_at,
        }
    }
}

impl FromTursoRow for BucketDto {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row_text(row, 0)?,
            client_id: row_text(row, 1)?,
            name: row_text(row, 2)?,
            label: row_text(row, 3)?,
            images_only: match row_integer(row, 4)? {
                1 => true,
                _ => false,
            },
            created_at: row_integer(row, 5)?,
        })
    }
}

pub const MAX_BUCKETS_PER_CLIENT: i32 = 50;

pub struct BucketRepo {
    db_pool: Connection,
}

impl BucketRepo {
    pub fn new(db_pool: Connection) -> Self {
        Self { db_pool }
    }

    pub async fn list(&self, client_id: &str) -> Result<Vec<BucketDto>> {
        // Each client/org can only have 1 bucket now, so we list by id
        let query = r#"
            SELECT
                id,
                client_id,
                name,
                label,
                images_only,
                created_at,
                updated_at
            FROM buckets
            WHERE client_id = :client_id AND deleted_at IS NULL
            ORDER BY name ASC
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":client_id", client_id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query(q_params).await.context(DbStatementSnafu)?;
        let items: Vec<BucketDto> = collect_rows(&mut rows).await?;

        Ok(items)
    }

    pub async fn create(&self, client_id: &str, data: &NewBucket) -> Result<BucketDto> {
        let today = chrono::Utc::now().timestamp();

        let query = r#"
            INSERT INTO buckets
            (
                id,
                client_id,
                name,
                label,
                images_only,
                created_at,
                updated_at,
                deleted_at
            )
            VALUES
            (
                :id,
                :client_id,
                :name,
                :label,
                :images_only,
                :created_at,
                NULL
            )
        "#;

        let id = generate_id();
        let images_only: i64 = if data.images_only { 1 } else { 0 };

        let mut params = new_query_params();
        params.push(text_param(":id", id.clone()));
        params.push(text_param(":client_id", client_id.to_owned()));
        params.push(text_param(":name", data.name.clone()));
        params.push(text_param(":label", data.label.clone()));
        params.push(integer_param(":images_only", images_only));
        params.push(integer_param(":created_at", today));
        params.push(integer_param(":updated_at", today));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        stmt.execute(params).await.context(DbStatementSnafu)?;

        Ok(BucketDto {
            id,
            client_id: client_id.to_owned(),
            name: data.name.clone(),
            label: data.label.clone(),
            images_only: data.images_only,
            created_at: today,
        })
    }

    pub async fn get(&self, id: &str) -> Result<Option<BucketDto>> {
        let query = r#"
            SELECT
                id,
                client_id,
                name,
                label,
                images_only,
                created_at,
                updated_at
            FROM buckets
            WHERE id = :id AND deleted_at IS NULL
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":id", id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<BucketDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn find_by_name(&self, client_id: &str, name: &str) -> Result<Option<BucketDto>> {
        let query = r#"
            SELECT
                id,
                client_id,
                name,
                label,
                images_only,
                created_at,
                updated_at
            FROM buckets
            WHERE
                client_id = :client_id
                AND name = :name
                AND deleted_at IS NULL
            LIMIT 1
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":client_id", client_id.to_owned()));
        q_params.push(text_param(":name", name.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        let dto: Option<BucketDto> = collect_row(row_result)?;
        Ok(dto)
    }

    pub async fn count_by_client(&self, client_id: &str) -> Result<i64> {
        let query = r#"
            SELECT COUNT(*) AS total_count
            FROM buckets
            WHERE client_id = :client_id AND deleted_at IS NULL
        "#
        .to_string();

        let mut q_params = new_query_params();
        q_params.push(text_param(":client_id", client_id.to_owned()));

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(q_params).await;
        collect_count(row_result)
    }

    pub async fn update(&self, id: &str, data: &UpdateBucket) -> Result<bool> {
        // Do not update if there is no data to update
        let Some(label) = data.label.clone() else {
            return Ok(false);
        };

        let query = r#"
            UPDATE buckets
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

    pub async fn delete(&self, id: &str) -> Result<()> {
        let today = chrono::Utc::now().timestamp();

        let query = r#"
            UPDATE buckets
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
                client_id,
                name,
                label,
                images_only,
                created_at,
                updated_at
            FROM buckets
            LIMIT 1
        "#
        .to_string();

        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row({}).await;
        let _: Option<BucketDto> = collect_row(row_result)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bucket() {
        let data = NewBucket {
            name: "hello-world".to_string(),
            label: "Hello World".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_ok());

        let data = NewBucket {
            name: "hello_world".to_string(),
            label: "Hello World".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());

        let data = NewBucket {
            name: "".to_string(),
            label: "".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());
    }
}
