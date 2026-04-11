use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use turso::{Connection, Row};
use validator::Validate;

use crate::Result;
use crate::turso_decode::{
    FromTursoRow, collect_count, collect_row, collect_rows, opt_row_text, row_integer, row_text,
};
use crate::turso_params::{integer_param, new_query_params, text_param};
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

pub const MAX_BUCKETS_PER_CLIENT: i32 = 50;

pub struct BucketRepo {
    db_pool: Connection,
}

impl BucketRepo {
    pub fn new(db_pool: Connection) -> Self {
        Self { db_pool }
    }

    async fn list(&self, client_id: &str) -> Result<Vec<BucketDto>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let client_id = client_id.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::buckets
                    .filter(dsl::client_id.eq(&client_id))
                    .select(Bucket::as_select())
                    .order(dsl::name.asc())
                    .load::<Bucket>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let items = select_res.context(DbQuerySnafu {
            table: "buckets".to_string(),
        })?;

        let dtos: Vec<BucketDto> = items.into_iter().map(|item| item.into()).collect();
        Ok(dtos)
    }

    async fn create(&self, client_id: &str, data: &NewBucket) -> Result<BucketDto> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;
        let data_copy = data.clone();
        let today = chrono::Utc::now().timestamp();
        let bucket = Bucket {
            id: generate_id(),
            client_id: client_id.to_string(),
            name: data_copy.name,
            label: data_copy.label,
            images_only: if data_copy.images_only { 1 } else { 0 },
            created_at: today,
        };

        let bucket_copy = bucket.clone();
        let insert_res = db
            .interact(move |conn| {
                diesel::insert_into(buckets::table)
                    .values(&bucket_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = insert_res.context(DbQuerySnafu {
            table: "buckets".to_string(),
        })?;

        Ok(bucket.into())
    }

    async fn get(&self, id: &str) -> Result<Option<BucketDto>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let bid = id.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::buckets
                    .find(bid)
                    .select(Bucket::as_select())
                    .first::<Bucket>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "buckets".to_string(),
        })?;

        Ok(item.map(|item| item.into()))
    }

    async fn find_by_name(&self, client_id: &str, name: &str) -> Result<Option<BucketDto>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let cid = client_id.to_string();
        let name_copy = name.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::buckets
                    .filter(dsl::client_id.eq(cid.as_str()))
                    .filter(dsl::name.eq(name_copy.as_str()))
                    .select(Bucket::as_select())
                    .first::<Bucket>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "buckets".to_string(),
        })?;

        Ok(item.map(|item| item.into()))
    }

    async fn count_by_client(&self, client_id: &str) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let cid = client_id.to_string();
        let count_res = db
            .interact(move |conn| {
                dsl::buckets
                    .filter(dsl::client_id.eq(cid.as_str()))
                    .select(count_star())
                    .get_result::<i64>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "buckets".to_string(),
        })?;

        Ok(count)
    }

    async fn update(&self, id: &str, data: &UpdateBucket) -> Result<bool> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        // Do not update if there is no data to update
        if data.label.is_none() {
            return Ok(false);
        }

        let data_copy = data.clone();
        let bid = id.to_string();
        let update_res = db
            .interact(move |conn| {
                diesel::update(dsl::buckets)
                    .filter(dsl::id.eq(bid.as_str()))
                    .set(data_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let item = update_res.context(DbQuerySnafu {
            table: "buckets".to_string(),
        })?;

        Ok(item > 0)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let bucket_id = id.to_string();
        let delete_res = db
            .interact(move |conn| {
                diesel::delete(dsl::buckets.filter(dsl::id.eq(bucket_id.as_str()))).execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = delete_res.context(DbQuerySnafu {
            table: "buckets".to_string(),
        })?;

        Ok(())
    }

    async fn test_read(&self) -> Result<()> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let selected_res = db
            .interact(move |conn| {
                dsl::buckets
                    .select(Bucket::as_select())
                    .first::<Bucket>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = selected_res.context(DbQuerySnafu {
            table: "buckets".to_string(),
        })?;

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
