use std::path::Path;

use snafu::ResultExt;
use turso::{Builder, Connection};

use crate::bucket::BucketRepo;
use crate::client::ClientRepo;
use crate::error::{DbBuilderSnafu, DbConnectSnafu};
use crate::user::UserRepo;

use crate::Result;

pub async fn create_db_pool(filename: &Path) -> Result<Connection> {
    let db = Builder::new_local(filename.to_str().expect("DB path is required"))
        .build()
        .await
        .context(DbBuilderSnafu)?;
    let conn = db.connect().context(DbConnectSnafu)?;

    // Enable MVCC
    conn.pragma_update("journal_mode", "'mvcc'")
        .await
        .context(DbConnectSnafu)?;

    Ok(conn)
}

pub struct DbMapper {
    pub buckets: BucketRepo,
    pub clients: ClientRepo,
    pub users: UserRepo,
}

pub async fn create_db_mapper(filename: &Path) -> Result<DbMapper> {
    let pool = create_db_pool(filename).await?;
    Ok(DbMapper {
        buckets: BucketRepo::new(pool.clone()),
        clients: ClientRepo::new(pool.clone()),
        users: UserRepo::new(pool.clone()),
    })
}
