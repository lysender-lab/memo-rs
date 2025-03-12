use deadpool_diesel::sqlite::Pool;

use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use tracing::error;
use validator::Validate;

use crate::dirs::{Dir, NewDir, UpdateDir};
use crate::files::count_dir_files;
use crate::schema::dirs::{self, dsl};
use crate::util::generate_id;
use crate::validators::flatten_errors;
use crate::web::pagination::Paginated;
use crate::{Error, Result};

use super::ListDirsParams;

const MAX_DIRS: i32 = 1000;
const MAX_PER_PAGE: i32 = 50;

pub async fn list_dirs(
    db_pool: &Pool,
    bucket_id: &str,
    params: &ListDirsParams,
) -> Result<Paginated<Dir>> {
    if let Err(errors) = params.validate() {
        return Err(Error::ValidationError(flatten_errors(&errors)));
    }
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let bid = bucket_id.to_string();

    let total_records = list_dirs_count(db_pool, bucket_id, params).await?;
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
    let conn_result = db
        .interact(move |conn| {
            let mut query = dsl::dirs.into_boxed();
            query = query.filter(dsl::bucket_id.eq(bid.as_str()));

            if let Some(keyword) = params_copy.keyword {
                if keyword.len() > 0 {
                    let pattern = format!("%{}%", keyword);
                    query =
                        query.filter(dsl::name.like(pattern.clone()).or(dsl::label.like(pattern)));
                }
            }
            query
                .limit(per_page as i64)
                .offset(offset)
                .select(Dir::as_select())
                .order(dsl::updated_at.desc())
                .load::<Dir>(conn)
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(items) => Ok(Paginated::new(items, page, per_page, total_records)),
            Err(e) => {
                error!("{e}");
                Err("Error reading directories".into())
            }
        },
        Err(e) => {
            error!("{e}");
            Err("Error using the db connection".into())
        }
    }
}

async fn list_dirs_count(db_pool: &Pool, bucket_id: &str, params: &ListDirsParams) -> Result<i64> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let bid = bucket_id.to_string();
    let params_copy = params.clone();

    let conn_result = db
        .interact(move |conn| {
            let mut query = dsl::dirs.into_boxed();
            query = query.filter(dsl::bucket_id.eq(bid.as_str()));
            if let Some(keyword) = params_copy.keyword {
                if keyword.len() > 0 {
                    let pattern = format!("%{}%", keyword);
                    query =
                        query.filter(dsl::name.like(pattern.clone()).or(dsl::label.like(pattern)));
                }
            }
            query.select(count_star()).get_result::<i64>(conn)
        })
        .await;

    match conn_result {
        Ok(count_res) => match count_res {
            Ok(count) => Ok(count),
            Err(e) => {
                error!("{}", e);
                Err("Error counting directories".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn create_dir(db_pool: &Pool, bucket_id: &str, data: &NewDir) -> Result<Dir> {
    if let Err(errors) = data.validate() {
        return Err(Error::ValidationError(flatten_errors(&errors)));
    }

    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    // Limit the number of directories per bucket
    let _ = match count_bucket_dirs(db_pool, bucket_id).await {
        Ok(count) => {
            if count >= MAX_DIRS as i64 {
                return Err(Error::ValidationError(
                    "Maximum number of dirs reached".to_string(),
                ));
            }
        }
        Err(e) => return Err(e),
    };

    // Directory name must be unique for the bucket
    if let Some(_) = find_bucket_dir(db_pool, bucket_id, data.name.as_str()).await? {
        return Err(Error::ValidationError(
            "Directory name already exists".to_string(),
        ));
    }

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
    let conn_result = db
        .interact(move |conn| {
            diesel::insert_into(dirs::table)
                .values(&dir_copy)
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(insert_res) => match insert_res {
            Ok(_) => Ok(dir),
            Err(e) => {
                error!("{}", e);
                Err("Error creating a directory".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn get_dir(pool: &Pool, id: &str) -> Result<Option<Dir>> {
    let Ok(db) = pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let did = id.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::dirs
                .find(did)
                .select(Dir::as_select())
                .first::<Dir>(conn)
                .optional()
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(item) => Ok(item),
            Err(e) => {
                error!("{e}");
                Err("Error reading directories".into())
            }
        },
        Err(e) => {
            error!("{e}");
            Err("Error using the db connection".into())
        }
    }
}

pub async fn find_bucket_dir(pool: &Pool, bucket_id: &str, name: &str) -> Result<Option<Dir>> {
    let Ok(db) = pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let bid = bucket_id.to_string();
    let name_copy = name.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::dirs
                .filter(dsl::bucket_id.eq(bid.as_str()))
                .filter(dsl::name.eq(name_copy.as_str()))
                .select(Dir::as_select())
                .first::<Dir>(conn)
                .optional()
        })
        .await;

    match conn_result {
        Ok(select_res) => match select_res {
            Ok(item) => Ok(item),
            Err(e) => {
                error!("{}", e);
                Err("Error finding dir".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn count_bucket_dirs(db_pool: &Pool, bucket_id: &str) -> Result<i64> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let bid = bucket_id.to_string();
    let conn_result = db
        .interact(move |conn| {
            dsl::dirs
                .filter(dsl::bucket_id.eq(bid.as_str()))
                .select(count_star())
                .get_result::<i64>(conn)
        })
        .await;

    match conn_result {
        Ok(count_res) => match count_res {
            Ok(count) => Ok(count),
            Err(e) => {
                error!("{}", e);
                Err("Error counting directories".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn update_dir(db_pool: &Pool, id: &str, data: &UpdateDir) -> Result<bool> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    if let Err(errors) = data.validate() {
        return Err(Error::ValidationError(flatten_errors(&errors)));
    }

    // Do not update if there is no data to update
    if data.label.is_none() {
        return Ok(false);
    }

    let data_copy = data.clone();
    let dir_id = id.to_string();
    let conn_result = db
        .interact(move |conn| {
            diesel::update(dsl::dirs)
                .filter(dsl::id.eq(dir_id.as_str()))
                .set(data_copy)
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(update_res) => match update_res {
            Ok(item) => Ok(item > 0),
            Err(e) => {
                error!("{}", e);
                Err("Error updating directory".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn update_dir_timestamp(db_pool: &Pool, id: &str, timestamp: i64) -> Result<bool> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    let dir_id = id.to_string();
    let conn_result = db
        .interact(move |conn| {
            diesel::update(dsl::dirs)
                .filter(dsl::id.eq(dir_id.as_str()))
                .set(dsl::updated_at.eq(timestamp))
                .execute(conn)
        })
        .await;

    match conn_result {
        Ok(update_res) => match update_res {
            Ok(item) => Ok(item > 0),
            Err(e) => {
                error!("{}", e);
                Err("Error updating directory timestamp".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}

pub async fn delete_dir(db_pool: &Pool, id: &str) -> Result<()> {
    let Ok(db) = db_pool.get().await else {
        return Err("Error getting db connection".into());
    };

    // Do not delete if there are still files inside
    let file_count = count_dir_files(db_pool, id).await?;
    if file_count > 0 {
        return Err(Error::ValidationError(
            "Cannot delete directory with files inside".to_string(),
        ));
    }

    let dir_id = id.to_string();
    let conn_result = db
        .interact(move |conn| {
            diesel::delete(dsl::dirs.filter(dsl::id.eq(dir_id.as_str()))).execute(conn)
        })
        .await;

    match conn_result {
        Ok(delete_res) => match delete_res {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("{}", e);
                Err("Error deleting directory".into())
            }
        },
        Err(e) => {
            error!("{}", e);
            Err("Error using the db connection".into())
        }
    }
}
