use async_trait::async_trait;

use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};
use validator::Validate;

use crate::Result;
use crate::error::{
    DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, MaxBucketsReachedSnafu, ValidationSnafu,
};
use crate::schema::buckets::{self, dsl};
use crate::state::AppState;
use memo::{bucket::BucketDto, utils::generate_id, validators::flatten_errors};

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize)]
#[diesel(table_name = crate::schema::buckets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Bucket {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub images_only: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct NewBucket {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::sluggable"))]
    pub name: String,

    pub images_only: bool,
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
            images_only: bucket.images_only == 1,
            created_at: bucket.created_at,
        }
    }
}

const MAX_BUCKETS_PER_CLIENT: i32 = 50;

pub async fn create_bucket(
    state: &AppState,
    client_id: &str,
    data: &NewBucket,
) -> Result<BucketDto> {
    let valid_res = data.validate();
    ensure!(
        valid_res.is_ok(),
        ValidationSnafu {
            msg: flatten_errors(&valid_res.unwrap_err()),
        }
    );

    // Limit the number of buckets per client
    let count = state.db.buckets.count_by_client(client_id).await?;
    ensure!(
        count < MAX_BUCKETS_PER_CLIENT as i64,
        MaxBucketsReachedSnafu
    );

    // Bucket name must be unique for the client
    let existing = state.db.buckets.find_by_name(client_id, &data.name).await?;
    ensure!(
        existing.is_none(),
        ValidationSnafu {
            msg: "Bucket name already exists".to_string(),
        }
    );

    // Validate against the cloud storage
    let _ = state.storage_client.read_bucket(&data.name).await?;

    state.db.buckets.create(client_id, data).await
}

pub async fn delete_bucket(state: &AppState, id: &str) -> Result<()> {
    // Do not delete if there are still directories inside
    let dir_count = state.db.dirs.count(id).await?;
    ensure!(
        dir_count == 0,
        ValidationSnafu {
            msg: "Cannot delete bucket with directories inside".to_string(),
        }
    );

    state.db.buckets.delete(id).await
}

#[async_trait]
pub trait BucketRepoable: Send + Sync {
    async fn list(&self, client_id: &str) -> Result<Vec<BucketDto>>;

    async fn create(&self, client_id: &str, data: &NewBucket) -> Result<BucketDto>;

    async fn get(&self, id: &str) -> Result<Option<BucketDto>>;

    async fn find_by_name(&self, client_id: &str, name: &str) -> Result<Option<BucketDto>>;

    async fn count_by_client(&self, client_id: &str) -> Result<i64>;

    async fn delete(&self, id: &str) -> Result<()>;

    async fn test_read(&self) -> Result<()>;
}

pub struct BucketRepo {
    db_pool: Pool,
}

impl BucketRepo {
    pub fn new(db_pool: Pool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl BucketRepoable for BucketRepo {
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
pub const TEST_BUCKET_ID: &'static str = "0196d1bbc22f79c89cdbc8beced0d2f0";

#[cfg(test)]
pub fn create_test_bucket() -> BucketDto {
    use crate::client::TEST_CLIENT_ID;
    let today = chrono::Utc::now().timestamp();

    BucketDto {
        id: TEST_BUCKET_ID.to_string(),
        name: "test-bucket".to_string(),
        client_id: TEST_CLIENT_ID.to_string(),
        images_only: true,
        created_at: today,
    }
}

#[cfg(test)]
pub struct BucketTestRepo {}

#[cfg(test)]
#[async_trait]
impl BucketRepoable for BucketTestRepo {
    async fn list(&self, client_id: &str) -> Result<Vec<BucketDto>> {
        let bucket = create_test_bucket();
        let buckets = vec![bucket];
        let filtered = buckets
            .into_iter()
            .filter(|x| x.client_id.as_str() == client_id)
            .collect();
        Ok(filtered)
    }

    async fn create(&self, _client_id: &str, _data: &NewBucket) -> Result<BucketDto> {
        Err("No supported".into())
    }

    async fn get(&self, id: &str) -> Result<Option<BucketDto>> {
        let bucket = create_test_bucket();
        let buckets = vec![bucket];
        let found = buckets.into_iter().find(|x| x.id.as_str() == id);
        Ok(found)
    }

    async fn find_by_name(&self, client_id: &str, name: &str) -> Result<Option<BucketDto>> {
        let buckets = self.list(client_id).await?;
        let found = buckets.into_iter().find(|x| x.name.as_str() == name);
        Ok(found)
    }

    async fn count_by_client(&self, client_id: &str) -> Result<i64> {
        let buckets = self.list(client_id).await?;
        Ok(buckets.len() as i64)
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Ok(())
    }

    async fn test_read(&self) -> Result<()> {
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
            images_only: false,
        };
        assert!(data.validate().is_ok());

        let data = NewBucket {
            name: "hello_world".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());

        let data = NewBucket {
            name: "".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());
    }
}
