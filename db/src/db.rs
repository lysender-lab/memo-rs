use std::path::Path;

use snafu::ResultExt;
use turso::{Builder, Connection};

use crate::any::AnyRepo;
use crate::bucket::BucketRepo;
use crate::dir::DirRepo;
use crate::error::{DbBuilderSnafu, DbConnectSnafu};
use crate::file::FileRepo;

use crate::Result;

pub async fn create_db_pool(filename: &Path) -> Result<Connection> {
    let db = Builder::new_local(filename.to_str().expect("DB path is required"))
        .build()
        .await
        .context(DbBuilderSnafu)?;
    let conn = db.connect().context(DbConnectSnafu)?;

    Ok(conn)
}

pub struct DbMapper {
    pub buckets: BucketRepo,
    pub dirs: DirRepo,
    pub files: FileRepo,
    pub any: AnyRepo,
}

pub async fn create_db_mapper(filename: &Path) -> Result<DbMapper> {
    let pool = create_db_pool(filename).await?;
    Ok(DbMapper {
        buckets: BucketRepo::new(pool.clone()),
        dirs: DirRepo::new(pool.clone()),
        files: FileRepo::new(pool.clone()),
        any: AnyRepo::new(pool.clone()),
    })
}
