//! Database-backed job workers.
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use db::DbMapper;
use memo::job::JobDto;

use super::handlers::dispatch;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const LEASE_DURATION: Duration = Duration::from_secs(60);
const RENEW_INTERVAL: Duration = Duration::from_secs(20);
const RECOVERY_INTERVAL: Duration = Duration::from_secs(30);
const RETRY_DELAY_SECONDS: i64 = 30;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

pub struct WorkerPool {
    shutdown: watch::Sender<bool>,
    supervisor: tokio::task::JoinHandle<()>,
}

impl WorkerPool {
    pub fn start(db: Arc<DbMapper>, count: usize) -> Self {
        let (shutdown, receiver) = watch::channel(false);
        let supervisor = tokio::spawn(async move {
            if count > 0 {
                info!(count, "Job worker pool started");
                dispatcher_loop(db, receiver, count).await;
            }
        });

        Self {
            shutdown,
            supervisor,
        }
    }

    pub async fn shutdown(mut self) {
        let _ = self.shutdown.send(true);
        if tokio::time::timeout(SHUTDOWN_TIMEOUT, &mut self.supervisor)
            .await
            .is_err()
        {
            warn!("Job worker shutdown timed out; abandoning remaining leases");
            self.supervisor.abort();
            let _ = self.supervisor.await;
        }
    }
}

async fn dispatcher_loop(db: Arc<DbMapper>, mut shutdown: watch::Receiver<bool>, count: usize) {
    let mut tasks = JoinSet::new();
    let mut recovery = tokio::time::interval(RECOVERY_INTERVAL);
    recovery.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut next_poll = tokio::time::Instant::now();
    loop {
        if *shutdown.borrow() {
            break;
        }
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = recovery.tick() => {
                if let Err(error) = db.jobs.recover_expired().await {
                    warn!(%error, "Unable to recover expired jobs");
                }
            }
            Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                if let Err(error) = result {
                    error!(%error, "Job worker task exited unexpectedly");
                }
            }
            _ = tokio::time::sleep_until(next_poll), if tasks.len() < count => {
                match db.jobs.claim_next(LEASE_DURATION).await {
                    Ok(Some(job)) => {
                        let db = db.clone();
                        tasks.spawn(async move { run_job(&db, job).await });
                        next_poll = tokio::time::Instant::now();
                    }
                    Ok(None) => next_poll = tokio::time::Instant::now() + POLL_INTERVAL,
                    Err(error) => {
                        warn!(%error, "Unable to claim job");
                        next_poll = tokio::time::Instant::now() + POLL_INTERVAL;
                    }
                }
            }
        }
    }
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            error!(%error, "Job worker task exited unexpectedly");
        }
    }
}

async fn run_job(db: &DbMapper, job: JobDto) {
    let Some(token) = job.lease_token.as_deref() else {
        error!(job_id = %job.id, "Claimed job has no lease token");
        return;
    };
    let handler = dispatch(&job);
    tokio::pin!(handler);
    let outcome = loop {
        tokio::select! {
            result = &mut handler => break result,
            _ = tokio::time::sleep(RENEW_INTERVAL) => {
                match db.jobs.extend_lease(&job.id, token, LEASE_DURATION).await {
                    Ok(true) => {}
                    Ok(false) => {
                        warn!(job_id = %job.id, "Job lease lost");
                        return;
                    }
                    Err(error) => {
                        warn!(job_id = %job.id, %error, "Unable to renew job lease");
                        return;
                    }
                }
            }
        }
    };
    let result = match outcome {
        Ok(()) => db.jobs.complete(&job.id, token).await,
        Err(message) => {
            let retry_at = chrono::Utc::now()
                .timestamp()
                .saturating_add(RETRY_DELAY_SECONDS);
            db.jobs.fail(&job.id, token, &message, retry_at).await
        }
    };
    match result {
        Ok(true) => {}
        Ok(false) => warn!(job_id = %job.id, "Job result not recorded because lease was lost"),
        Err(error) => warn!(job_id = %job.id, %error, "Unable to record job result"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{enqueue, handlers::LOGGER_JOB_TYPE};
    use memo::job::{JobStatus, NewJobDto};

    #[tokio::test]
    async fn workers_execute_and_fail_invalid_jobs() {
        let dir =
            std::env::temp_dir().join(format!("memo-worker-test-{}", memo::utils::generate_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("jobs.db");
        let conn = db::db::create_db_pool(&path).await.unwrap();
        for statement in
            include_str!("../../../db/db/migrations/memo/12-create-jobs.sql").split(';')
        {
            if !statement.trim().is_empty() {
                conn.execute(statement.trim(), ()).await.unwrap();
            }
        }
        drop(conn);
        let db = Arc::new(db::create_db_mapper(&path, 4).await.unwrap());
        let job = enqueue(
            &db,
            NewJobDto {
                job_type: LOGGER_JOB_TYPE.into(),
                payload: r#"{"message":"a test message"}"#.into(),
                max_attempts: 1,
                available_at: chrono::Utc::now().timestamp(),
            },
        )
        .await
        .unwrap();
        let invalid = enqueue(
            &db,
            NewJobDto {
                job_type: LOGGER_JOB_TYPE.into(),
                payload: "{}".into(),
                max_attempts: 1,
                available_at: chrono::Utc::now().timestamp(),
            },
        )
        .await
        .unwrap();
        let unknown = enqueue(
            &db,
            NewJobDto {
                job_type: "unknown".into(),
                payload: "{}".into(),
                max_attempts: 1,
                available_at: chrono::Utc::now().timestamp(),
            },
        )
        .await
        .unwrap();
        let disabled = WorkerPool::start(db.clone(), 0);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            db.jobs.get(&job.id).await.unwrap().unwrap().status,
            JobStatus::Pending
        );
        disabled.shutdown().await;
        let pool = WorkerPool::start(db.clone(), 2);
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let a = db.jobs.get(&job.id).await.unwrap().unwrap();
                let b = db.jobs.get(&unknown.id).await.unwrap().unwrap();
                let c = db.jobs.get(&invalid.id).await.unwrap().unwrap();
                if a.status == JobStatus::Completed
                    && b.status == JobStatus::Failed
                    && c.status == JobStatus::Failed
                {
                    assert_eq!(b.last_error.as_deref(), Some("Unknown job type"));
                    assert_eq!(c.last_error.as_deref(), Some("Invalid logger job payload"));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        pool.shutdown().await;
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
