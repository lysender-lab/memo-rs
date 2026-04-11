use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use memo::utils::generate_id;
use snafu::ResultExt;
use turso::{Builder, Connection, Value};

use crate::db::create_db_mapper;
use crate::error::{DbBuilderSnafu, DbConnectSnafu, DbPrepareSnafu, DbStatementSnafu, IoSnafu};
use crate::{DbMapper, Result};

const MIGRATIONS: &[&str] = &[
    include_str!("../db/migrations/02-create-clients.sql"),
    include_str!("../db/migrations/03-create-buckets.sql"),
    include_str!("../db/migrations/04-create-dirs.sql"),
    include_str!("../db/migrations/05-create-users.sql"),
    include_str!("../db/migrations/06-create-files.sql"),
];

pub struct TestCtx {
    db_dir: PathBuf,
    pub db: Arc<DbMapper>,
}

impl Drop for TestCtx {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.db_dir);
    }
}

impl TestCtx {
    pub async fn new(test_name: &str) -> Result<Self> {
        let root = test_root_dir()?;
        let unique = generate_id();
        let db_dir = root.join("test").join(format!("{}-{}", test_name, unique));
        let db_file = db_dir.join("yaas.db");

        fs::create_dir_all(&db_dir).context(IoSnafu)?;

        let conn = create_connection(&db_file).await?;
        run_migrations(&conn).await?;

        let mapper = create_db_mapper(db_file.as_path()).await?;

        Ok(Self {
            db_dir,
            db: Arc::new(mapper),
        })
    }
}

fn test_root_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("DATABASE_DIR")
        && !dir.is_empty()
    {
        return Ok(PathBuf::from(dir));
    }

    Ok(std::env::temp_dir().join("yaas"))
}

async fn create_connection(filename: &Path) -> Result<Connection> {
    let db = Builder::new_local(filename.to_str().expect("DB path is required"))
        .build()
        .await
        .context(DbBuilderSnafu)?;
    let conn = db.connect().context(DbConnectSnafu)?;

    Ok(conn)
}

async fn run_migrations(conn: &Connection) -> Result<()> {
    for migration in MIGRATIONS {
        for stmt in migration.split(';') {
            let sql = stmt.trim();
            if sql.is_empty() {
                continue;
            }

            let mut prepared = conn.prepare(sql).await.context(DbPrepareSnafu)?;
            prepared
                .execute(Vec::<(String, Value)>::new())
                .await
                .context(DbStatementSnafu)?;
        }
    }

    Ok(())
}
