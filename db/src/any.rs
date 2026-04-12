use snafu::ResultExt;
use turso::Connection;
use turso::Value;

use crate::Result;
use crate::error::{DbPrepareSnafu, DbStatementSnafu};
use crate::turso_decode::{FromTursoRow, collect_count, collect_rows};

/// Allows running artitrary queries/executions against any table.
pub struct AnyRepo {
    db_pool: Connection,
}

impl AnyRepo {
    pub fn new(db_pool: Connection) -> Self {
        Self { db_pool }
    }

    /// Return rows from any query
    pub async fn query<T: FromTursoRow>(
        &self,
        query: String,
        params: Vec<(String, Value)>,
    ) -> Result<Vec<T>> {
        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let mut rows = stmt.query(params).await.context(DbStatementSnafu)?;

        Ok(collect_rows(&mut rows).await?)
    }

    /// Return count result from any count query
    pub async fn count_query(&self, query: String, params: Vec<(String, Value)>) -> Result<i64> {
        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let row_result = stmt.query_row(params).await;
        collect_count(row_result)
    }

    /// Execute any query that doesn't return rows
    pub async fn execute(&self, query: String, params: Vec<(String, Value)>) -> Result<bool> {
        let mut stmt = self.db_pool.prepare(query).await.context(DbPrepareSnafu)?;
        let affected = stmt.execute(params).await.context(DbStatementSnafu)?;
        Ok(affected > 0)
    }
}
