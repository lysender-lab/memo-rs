use snafu::ResultExt;
use turso::Value;
use turso::{Connection, Rows};

use crate::Result;
use crate::error::{DbPrepareSnafu, DbStatementSnafu};

/// Allows running artitrary queries/executions against any table.
pub struct AnyRepo {
    db_pool: Connection,
}

impl AnyRepo {
    pub fn new(db_pool: Connection) -> Self {
        Self { db_pool }
    }

    pub async fn query(&self, query: String, params: Vec<(String, Value)>) -> Result<Rows> {
        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let rows = stmt.query(params).await.context(DbStatementSnafu)?;

        Ok(rows)
    }

    pub async fn execute(&self, query: String, params: Vec<(String, Value)>) -> Result<bool> {
        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let affected = stmt.execute(params).await.context(DbStatementSnafu)?;
        Ok(affected > 0)
    }
}
