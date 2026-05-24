use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use snafu::{OptionExt, ResultExt, ensure};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;
use tracing::warn;
use turso::{Builder, Connection};

use crate::error::{DbBuilderSnafu, DbConnectSnafu, DbExecuteSnafu, DbPoolConfigSnafu};
use crate::{Error, Result};

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct DbPool {
    pool: Arc<ConnectionPool>,
}

struct ConnectionPool {
    size: usize,
    idle_connections: Mutex<Vec<Connection>>,
    permits: Arc<Semaphore>,
}

impl DbPool {
    pub async fn new(filename: &Path, pool_size: usize) -> Result<Self> {
        ensure!(
            pool_size > 0,
            DbPoolConfigSnafu {
                msg: "pool_size must be greater than zero".to_string(),
            }
        );

        let file_str = filename.to_str().context(DbPoolConfigSnafu {
            msg: "DB path must be a valid UTF-8 string".to_string(),
        })?;

        let db = Builder::new_local(file_str)
            .build()
            .await
            .context(DbBuilderSnafu)?;

        let mut idle_connections = Vec::with_capacity(pool_size);

        for _ in 0..pool_size {
            let conn = db.connect().context(DbConnectSnafu)?;

            conn.pragma_update("journal_mode", "'mvcc'")
                .await
                .context(DbExecuteSnafu)?;

            idle_connections.push(conn);
        }

        Ok(Self {
            pool: Arc::new(ConnectionPool {
                size: pool_size,
                idle_connections: Mutex::new(idle_connections),
                permits: Arc::new(Semaphore::new(pool_size)),
            }),
        })
    }

    pub async fn acquire(&self) -> Result<PooledConnection> {
        self.acquire_with_timeout(ACQUIRE_TIMEOUT).await
    }

    async fn acquire_with_timeout(&self, acquire_timeout: Duration) -> Result<PooledConnection> {
        let permit_result = timeout(acquire_timeout, self.pool.permits.clone().acquire_owned())
            .await
            .map_err(|source| Error::DbPoolAcquireTimeout { source })?;

        let permit = permit_result.map_err(|_| Error::DbPoolState {
            msg: "Semaphore closed while acquiring connection".to_string(),
        })?;

        let conn = {
            let mut idle = match self.pool.idle_connections.lock() {
                Ok(idle) => idle,
                Err(poisoned) => {
                    warn!("idle_connections mutex poisoned while acquiring; recovering pool state");
                    poisoned.into_inner()
                }
            };

            idle.pop().ok_or_else(|| Error::DbPoolState {
                msg: "Permit acquired but no idle connection was available".to_string(),
            })?
        };

        Ok(PooledConnection {
            conn: Some(conn),
            inner: Arc::clone(&self.pool),
            _permit: permit,
        })
    }

    #[allow(dead_code)]
    pub fn idle_count(&self) -> usize {
        self.pool
            .idle_connections
            .lock()
            .map(|idle| idle.len())
            .unwrap_or(0)
    }

    #[allow(dead_code)]
    pub fn in_use_count(&self) -> usize {
        self.pool.size.saturating_sub(self.idle_count())
    }
}

pub struct PooledConnection {
    conn: Option<Connection>,
    inner: Arc<ConnectionPool>,
    _permit: OwnedSemaphorePermit,
}

impl Deref for PooledConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.conn
            .as_ref()
            .expect("pooled connection must be available while guard is alive")
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn
            .as_mut()
            .expect("pooled connection must be available while guard is alive")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            let mut idle = match self.inner.idle_connections.lock() {
                Ok(idle) => idle,
                Err(poisoned) => {
                    warn!("idle_connections mutex poisoned while releasing; recovering pool state");
                    poisoned.into_inner()
                }
            };

            idle.push(conn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use memo::utils::{IdPrefix, generate_prefixed_id};
    use tokio::task::yield_now;

    fn temp_db_path(test_name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let id = generate_prefixed_id(IdPrefix::Any);
        path.push(format!("turso-concurrency-{test_name}-{id}.db"));
        path
    }

    fn cleanup_db_files(path: &Path) {
        let mut candidates = vec![path.to_path_buf()];

        if let Some(base) = path.to_str() {
            candidates.push(PathBuf::from(format!("{base}-wal")));
            candidates.push(PathBuf::from(format!("{base}-shm")));
            candidates.push(PathBuf::from(format!("{base}-log")));
        }

        for file in candidates {
            let _ = std::fs::remove_file(file);
        }
    }

    #[tokio::test]
    async fn acquire_and_release_updates_counts() {
        let db_path = temp_db_path("counts");
        let pool = DbPool::new(db_path.as_path(), 2)
            .await
            .expect("pool to build");

        assert_eq!(pool.idle_count(), 2);
        assert_eq!(pool.in_use_count(), 0);

        let conn1 = pool.acquire().await.expect("first acquire");
        assert_eq!(pool.idle_count(), 1);
        assert_eq!(pool.in_use_count(), 1);

        let conn2 = pool.acquire().await.expect("second acquire");
        assert_eq!(pool.idle_count(), 0);
        assert_eq!(pool.in_use_count(), 2);

        drop(conn1);
        assert_eq!(pool.idle_count(), 1);
        assert_eq!(pool.in_use_count(), 1);

        drop(conn2);
        assert_eq!(pool.idle_count(), 2);
        assert_eq!(pool.in_use_count(), 0);

        cleanup_db_files(db_path.as_path());
    }

    #[tokio::test]
    async fn dropped_connection_unblocks_waiting_acquire() {
        let db_path = temp_db_path("unblocks");
        let pool = DbPool::new(db_path.as_path(), 1)
            .await
            .expect("pool to build");

        let conn = pool.acquire().await.expect("initial acquire");
        let waiting_pool = pool.clone();
        let waiting = tokio::spawn(async move { waiting_pool.acquire().await });

        yield_now().await;
        assert_eq!(pool.idle_count(), 0);
        assert_eq!(pool.in_use_count(), 1);

        drop(conn);

        let reacquired = tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("waiter should finish quickly")
            .expect("join should succeed")
            .expect("acquire should succeed after drop");

        assert_eq!(pool.idle_count(), 0);
        assert_eq!(pool.in_use_count(), 1);

        drop(reacquired);
        assert_eq!(pool.idle_count(), 1);
        assert_eq!(pool.in_use_count(), 0);

        cleanup_db_files(db_path.as_path());
    }

    #[tokio::test]
    async fn acquire_times_out_when_pool_is_exhausted() {
        let db_path = temp_db_path("timeout");
        let pool = DbPool::new(db_path.as_path(), 1)
            .await
            .expect("pool to build");

        let _conn = pool.acquire().await.expect("initial acquire");
        let waiting_pool = pool.clone();
        let waiting = tokio::spawn(async move {
            waiting_pool
                .acquire_with_timeout(Duration::from_millis(50))
                .await
        });

        yield_now().await;

        let res = waiting.await.expect("join should succeed");
        assert!(matches!(res, Err(Error::DbPoolAcquireTimeout { .. })));

        cleanup_db_files(db_path.as_path());
    }

    #[tokio::test]
    async fn new_rejects_zero_pool_size() {
        let db_path = temp_db_path("zero-size");
        let res = DbPool::new(db_path.as_path(), 0).await;

        assert!(matches!(res, Err(Error::DbPoolConfig { .. })));

        cleanup_db_files(db_path.as_path());
    }

    #[tokio::test]
    async fn acquire_recovers_after_idle_mutex_poisoning() {
        let db_path = temp_db_path("poison-recover");
        let pool = DbPool::new(db_path.as_path(), 1)
            .await
            .expect("pool to build");

        let poisoned_pool = pool.clone();
        let join_res = std::thread::spawn(move || {
            let _idle = poisoned_pool
                .pool
                .idle_connections
                .lock()
                .expect("lock should succeed before poison");
            panic!("poison mutex intentionally for test");
        })
        .join();

        assert!(join_res.is_err(), "poisoning thread should panic");

        let conn = pool
            .acquire()
            .await
            .expect("acquire should recover after poison");
        drop(conn);

        let reacquired = tokio::time::timeout(Duration::from_secs(1), pool.acquire())
            .await
            .expect("reacquire should finish quickly")
            .expect("reacquire should recover after poison");
        drop(reacquired);

        cleanup_db_files(db_path.as_path());
    }
}
