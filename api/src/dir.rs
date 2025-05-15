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
    DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, MaxDirsReachedSnafu, ValidationSnafu,
};
use crate::schema::dirs::{self, dsl};
use crate::state::AppState;
use memo::pagination::Paginated;
use memo::utils::generate_id;
use memo::validators::flatten_errors;

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::dirs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Dir {
    pub id: String,
    pub bucket_id: String,
    pub name: String,
    pub label: String,
    pub file_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct NewDir {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "memo::validators::sluggable"))]
    pub name: String,

    #[validate(length(min = 1, max = 60))]
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Validate, AsChangeset)]
#[diesel(table_name = crate::schema::dirs)]
pub struct UpdateDir {
    #[validate(length(min = 1, max = 100))]
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

const MAX_DIRS: i32 = 1000;
const MAX_PER_PAGE: i32 = 50;

pub async fn delete_dir(state: &AppState, id: &str) -> Result<()> {
    // Do not delete if there are still files inside
    let file_count = state.db.files.count_by_dir(id).await?;
    ensure!(
        file_count == 0,
        ValidationSnafu {
            msg: "Cannot delete directory with files inside".to_string(),
        }
    );

    state.db.dirs.delete(id).await
}

#[async_trait]
pub trait DirRepoable: Send + Sync {
    async fn list(&self, bucket_id: &str, params: &ListDirsParams) -> Result<Paginated<Dir>>;

    async fn count(&self, bucket_id: &str) -> Result<i64>;

    async fn create(&self, bucket_id: &str, data: &NewDir) -> Result<Dir>;

    async fn get(&self, id: &str) -> Result<Option<Dir>>;

    async fn find_by_name(&self, bucket_id: &str, name: &str) -> Result<Option<Dir>>;

    async fn update(&self, id: &str, data: &UpdateDir) -> Result<bool>;

    async fn update_timestamp(&self, id: &str, timestamp: i64) -> Result<bool>;

    async fn delete(&self, id: &str) -> Result<()>;
}

pub struct DirRepo {
    db_pool: Pool,
}

impl DirRepo {
    pub fn new(db_pool: Pool) -> Self {
        Self { db_pool }
    }

    async fn listing_count(&self, bucket_id: &str, params: &ListDirsParams) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let bid = bucket_id.to_string();
        let params_copy = params.clone();

        let count_res = db
            .interact(move |conn| {
                let mut query = dsl::dirs.into_boxed();
                query = query.filter(dsl::bucket_id.eq(bid.as_str()));
                if let Some(keyword) = params_copy.keyword {
                    if keyword.len() > 0 {
                        let pattern = format!("%{}%", keyword);
                        query = query
                            .filter(dsl::name.like(pattern.clone()).or(dsl::label.like(pattern)));
                    }
                }
                query.select(count_star()).get_result::<i64>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(count)
    }
}

#[async_trait]
impl DirRepoable for DirRepo {
    async fn list(&self, bucket_id: &str, params: &ListDirsParams) -> Result<Paginated<Dir>> {
        let valid_res = params.validate();
        ensure!(
            valid_res.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&valid_res.unwrap_err()),
            }
        );

        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let bid = bucket_id.to_string();

        let total_records = self.listing_count(bucket_id, params).await?;
        let mut page: i32 = 1;
        let mut per_page: i32 = MAX_PER_PAGE;
        let mut offset: i64 = 0;

        if let Some(per_page_param) = params.per_page {
            if per_page_param > 0 && per_page_param <= MAX_PER_PAGE {
                per_page = per_page_param;
            }
        }

        let total_pages: i64 = (total_records as f64 / per_page as f64).ceil() as i64;

        if let Some(p) = params.page {
            let p64 = p as i64;
            if p64 > 0 && p64 <= total_pages {
                page = p;
                offset = (p64 - 1) * per_page as i64;
            }
        }

        // Do not query if we already know there are no records
        if total_pages == 0 {
            return Ok(Paginated::new(Vec::new(), page, per_page, total_records));
        }

        let params_copy = params.clone();
        let select_res = db
            .interact(move |conn| {
                let mut query = dsl::dirs.into_boxed();
                query = query.filter(dsl::bucket_id.eq(bid.as_str()));

                if let Some(keyword) = params_copy.keyword {
                    if keyword.len() > 0 {
                        let pattern = format!("%{}%", keyword);
                        query = query
                            .filter(dsl::name.like(pattern.clone()).or(dsl::label.like(pattern)));
                    }
                }
                query
                    .limit(per_page as i64)
                    .offset(offset)
                    .select(Dir::as_select())
                    .order(dsl::updated_at.desc())
                    .load::<Dir>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let items = select_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(Paginated::new(items, page, per_page, total_records))
    }

    async fn count(&self, bucket_id: &str) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let bid = bucket_id.to_string();
        let count_res = db
            .interact(move |conn| {
                dsl::dirs
                    .filter(dsl::bucket_id.eq(bid.as_str()))
                    .select(count_star())
                    .get_result::<i64>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(count)
    }

    async fn create(&self, bucket_id: &str, data: &NewDir) -> Result<Dir> {
        let valid_res = data.validate();
        ensure!(
            valid_res.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&valid_res.unwrap_err()),
            }
        );

        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        // Limit the number of directories per bucket
        let count = self.count(bucket_id).await?;
        ensure!(count < MAX_DIRS as i64, MaxDirsReachedSnafu,);

        // Directory name must be unique for the bucket
        let existing = self.find_by_name(bucket_id, data.name.as_str()).await?;
        ensure!(
            existing.is_none(),
            ValidationSnafu {
                msg: "Directory name already exists".to_string(),
            }
        );

        let data_copy = data.clone();
        let today = chrono::Utc::now().timestamp();
        let dir = Dir {
            id: generate_id(),
            bucket_id: bucket_id.to_string(),
            name: data_copy.name,
            label: data_copy.label,
            file_count: 0,
            created_at: today,
            updated_at: today,
        };

        let dir_copy = dir.clone();
        let insert_res = db
            .interact(move |conn| {
                diesel::insert_into(dirs::table)
                    .values(&dir_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = insert_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(dir)
    }

    async fn get(&self, id: &str) -> Result<Option<Dir>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = id.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::dirs
                    .find(did)
                    .select(Dir::as_select())
                    .first::<Dir>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(item)
    }

    async fn find_by_name(&self, bucket_id: &str, name: &str) -> Result<Option<Dir>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let bid = bucket_id.to_string();
        let name_copy = name.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::dirs
                    .filter(dsl::bucket_id.eq(bid.as_str()))
                    .filter(dsl::name.eq(name_copy.as_str()))
                    .select(Dir::as_select())
                    .first::<Dir>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(item)
    }

    async fn update(&self, id: &str, data: &UpdateDir) -> Result<bool> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let errors = data.validate();
        ensure!(
            errors.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&errors.unwrap_err()),
            }
        );

        // Do not update if there is no data to update
        if data.label.is_none() {
            return Ok(false);
        }

        let data_copy = data.clone();
        let dir_id = id.to_string();
        let update_res = db
            .interact(move |conn| {
                diesel::update(dsl::dirs)
                    .filter(dsl::id.eq(dir_id.as_str()))
                    .set(data_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let item = update_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(item > 0)
    }

    async fn update_timestamp(&self, id: &str, timestamp: i64) -> Result<bool> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let dir_id = id.to_string();
        let update_res = db
            .interact(move |conn| {
                diesel::update(dsl::dirs)
                    .filter(dsl::id.eq(dir_id.as_str()))
                    .set(dsl::updated_at.eq(timestamp))
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let item = update_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(item > 0)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        // TODO: Validate at service call level
        // Do not delete if there are still files inside
        // let file_count = count_dir_files(db_pool, id).await?;
        // ensure!(
        //     file_count == 0,
        //     ValidationSnafu {
        //         msg: "Cannot delete directory with files inside".to_string(),
        //     }
        // );

        let dir_id = id.to_string();
        let delete_res = db
            .interact(move |conn| {
                diesel::delete(dsl::dirs.filter(dsl::id.eq(dir_id.as_str()))).execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = delete_res.context(DbQuerySnafu {
            table: "dirs".to_string(),
        })?;

        Ok(())
    }
}

#[cfg(test)]
pub const TEST_DIR_ID: &'static str = "0196d1c6bdc97ac895e4e141b9f46b3a";

#[cfg(test)]
pub fn create_test_dir() -> Dir {
    use crate::bucket::TEST_BUCKET_ID;
    let today = chrono::Utc::now().timestamp();

    Dir {
        id: TEST_DIR_ID.to_string(),
        bucket_id: TEST_BUCKET_ID.to_string(),
        name: "test-dir".to_string(),
        label: "Test Dir".to_string(),
        file_count: 0,
        created_at: today.clone(),
        updated_at: today,
    }
}

#[cfg(test)]
pub struct DirTestRepo {}

#[cfg(test)]
#[async_trait]
impl DirRepoable for DirTestRepo {
    async fn list(&self, bucket_id: &str, _params: &ListDirsParams) -> Result<Paginated<Dir>> {
        let dir = create_test_dir();
        let dirs = vec![dir];
        let total_records = dirs.len() as i64;
        let filtered = dirs
            .into_iter()
            .filter(|x| {
                if x.bucket_id.as_str() == bucket_id {
                    // Do not apply params for now
                    return true;
                }
                false
            })
            .collect();

        Ok(Paginated::new(filtered, 1, 10, total_records))
    }

    async fn count(&self, bucket_id: &str) -> Result<i64> {
        let dirs = self
            .list(
                bucket_id,
                &ListDirsParams {
                    page: None,
                    per_page: None,
                    keyword: None,
                },
            )
            .await?;
        Ok(dirs.meta.total_records)
    }

    async fn create(&self, _bucket_id: &str, _data: &NewDir) -> Result<Dir> {
        Err("Not supported".into())
    }

    async fn get(&self, id: &str) -> Result<Option<Dir>> {
        let dir = create_test_dir();
        let dirs = vec![dir];
        let found = dirs.into_iter().find(|x| x.id.as_str() == id);
        Ok(found)
    }

    async fn find_by_name(&self, bucket_id: &str, name: &str) -> Result<Option<Dir>> {
        let dirs = self
            .list(
                bucket_id,
                &ListDirsParams {
                    page: None,
                    per_page: None,
                    keyword: None,
                },
            )
            .await?;
        let found = dirs.data.into_iter().find(|x| x.name.as_str() == name);
        Ok(found)
    }

    async fn update(&self, _id: &str, _data: &UpdateDir) -> Result<bool> {
        Ok(true)
    }

    async fn update_timestamp(&self, _id: &str, _timestamp: i64) -> Result<bool> {
        Ok(true)
    }

    async fn delete(&self, _id: &str) -> Result<()> {
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
