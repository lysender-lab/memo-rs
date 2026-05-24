use std::path::Path;
use std::sync::Arc;

use snafu::ResultExt;
use turso::{Builder, Connection};

use crate::any::AnyRepo;
use crate::db_pool::DbPool;
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

    conn.pragma_update("journal_mode", "'mvcc'")
        .await
        .context(DbConnectSnafu)?;

    Ok(conn)
}

pub struct DbMapper {
    pub dirs: DirRepo,
    pub files: FileRepo,
    pub any: AnyRepo,
}

pub struct LogsDbMapper {
    pub any: AnyRepo,
}

pub async fn create_db_mapper(filename: &Path, pool_size: usize) -> Result<DbMapper> {
    let pool = DbPool::new(filename, pool_size).await?;
    let arc_pool = Arc::new(pool);

    Ok(DbMapper {
        dirs: DirRepo::new(arc_pool.clone()),
        files: FileRepo::new(arc_pool.clone()),
        any: AnyRepo::new(arc_pool),
    })
}

pub async fn create_logs_db_mapper(filename: &Path) -> Result<LogsDbMapper> {
    let pool = DbPool::new(filename, 1).await?;
    let arc_pool = Arc::new(pool);
    Ok(LogsDbMapper {
        any: AnyRepo::new(arc_pool),
    })
}
