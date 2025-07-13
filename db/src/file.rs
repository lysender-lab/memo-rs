use async_trait::async_trait;

use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use memo::dir::DirDto;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};
use std::path::PathBuf;
use validator::Validate;

use crate::Result;
use crate::error::{DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, ValidationSnafu};

use crate::schema::files::{self, dsl};
use memo::file::{FileDto, ImgVersionDto};
use memo::pagination::Paginated;
use memo::validators::flatten_errors;

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize)]
#[diesel(table_name = crate::schema::files)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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

#[derive(Debug, Clone)]
pub struct PhotoExif {
    pub orientation: u32,
    pub img_taken_at: Option<i64>,
}

impl Default for PhotoExif {
    fn default() -> Self {
        Self {
            orientation: 1,
            img_taken_at: None,
        }
    }
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

/// Convert FileDto to File
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

/// Convert File to FileDto
impl From<FileObject> for FileDto {
    fn from(file: FileObject) -> Self {
        let img_versions = match file.img_versions {
            Some(versions_str) => {
                let versions: Vec<ImgVersionDto> = versions_str
                    .split(',')
                    .filter_map(|s| s.parse::<ImgVersionDto>().ok())
                    .collect();

                if versions.len() > 0 {
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

pub const MAX_PER_PAGE: i32 = 50;
pub const MAX_FILES: i32 = 1000;

#[async_trait]
pub trait FileStore: Send + Sync {
    async fn list(&self, dir: &DirDto, params: &ListFilesParams) -> Result<Paginated<FileObject>>;

    async fn create(&self, file_dto: FileDto) -> Result<FileObject>;

    async fn get(&self, id: &str) -> Result<Option<FileObject>>;

    async fn find_by_name(&self, dir_id: &str, name: &str) -> Result<Option<FileObject>>;

    async fn count_by_dir(&self, dir_id: &str) -> Result<i64>;

    async fn delete(&self, id: &str) -> Result<()>;
}

pub struct FileRepo {
    db_pool: Pool,
}

impl FileRepo {
    pub fn new(db_pool: Pool) -> Self {
        Self { db_pool }
    }

    pub async fn listing_count(&self, dir_id: &str, params: &ListFilesParams) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = dir_id.to_string();
        let params_copy = params.clone();

        let count_res = db
            .interact(move |conn| {
                let mut query = dsl::files.into_boxed();
                query = query.filter(dsl::dir_id.eq(did.as_str()));
                if let Some(keyword) = params_copy.keyword {
                    if keyword.len() > 0 {
                        let pattern = format!("%{}%", keyword);
                        query = query.filter(dsl::name.like(pattern));
                    }
                }
                query.select(count_star()).get_result::<i64>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(count)
    }
}

#[async_trait]
impl FileStore for FileRepo {
    async fn list(&self, dir: &DirDto, params: &ListFilesParams) -> Result<Paginated<FileObject>> {
        let errors = params.validate();
        ensure!(
            errors.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&errors.unwrap_err()),
            }
        );

        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = dir.id.clone();

        let total_records = self.listing_count(&dir.id, params).await?;
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
                let mut query = dsl::files.into_boxed();
                query = query.filter(dsl::dir_id.eq(did.as_str()));

                if let Some(keyword) = params_copy.keyword {
                    if keyword.len() > 0 {
                        let pattern = format!("%{}%", keyword);
                        query = query.filter(dsl::name.like(pattern));
                    }
                }
                query
                    .limit(per_page as i64)
                    .offset(offset)
                    .select(FileObject::as_select())
                    .order(dsl::created_at.desc())
                    .load::<FileObject>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let items = select_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(Paginated::new(items, page, per_page, total_records))
    }

    async fn create(&self, file_dto: FileDto) -> Result<FileObject> {
        let file_db_pool = self.db_pool.clone();
        let db = file_db_pool.get().await.context(DbPoolSnafu)?;

        let file: FileObject = file_dto.clone().into();
        let file_copy = file.clone();

        let insert_res = db
            .interact(move |conn| {
                diesel::insert_into(files::table)
                    .values(&file_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = insert_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(file)
    }

    async fn get(&self, id: &str) -> Result<Option<FileObject>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let fid = id.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::files
                    .find(fid)
                    .select(FileObject::as_select())
                    .first::<FileObject>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(item)
    }

    async fn find_by_name(&self, dir_id: &str, name: &str) -> Result<Option<FileObject>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = dir_id.to_string();
        let name_copy = name.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::files
                    .filter(dsl::dir_id.eq(did.as_str()))
                    .filter(dsl::name.eq(name_copy.as_str()))
                    .select(FileObject::as_select())
                    .first::<FileObject>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(item)
    }

    async fn count_by_dir(&self, dir_id: &str) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = dir_id.to_string();
        let count_res = db
            .interact(move |conn| {
                dsl::files
                    .filter(dsl::dir_id.eq(did.as_str()))
                    .select(count_star())
                    .get_result::<i64>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(count)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let fid = id.to_string();
        let delete_res = db
            .interact(move |conn| diesel::delete(dsl::files.filter(dsl::id.eq(fid))).execute(conn))
            .await
            .context(DbInteractSnafu)?;

        let _ = delete_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(())
    }
}

#[cfg(feature = "test")]
pub struct FileTestRepo {}

#[cfg(feature = "test")]
#[async_trait]
impl FileStore for FileTestRepo {
    async fn list(
        &self,
        _dir: &DirDto,
        _params: &ListFilesParams,
    ) -> Result<Paginated<FileObject>> {
        Ok(Paginated::new(vec![], 1, 10, 0))
    }

    async fn create(&self, _file_dto: FileDto) -> Result<FileObject> {
        Err("Not supported".into())
    }

    async fn get(&self, _id: &str) -> Result<Option<FileObject>> {
        Ok(None)
    }

    async fn find_by_name(&self, _dir_id: &str, _name: &str) -> Result<Option<FileObject>> {
        Ok(None)
    }

    async fn count_by_dir(&self, _dir_id: &str) -> Result<i64> {
        Ok(0)
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}
