//! Database-backed job workers and job dispatch.
use std::sync::Arc;
use std::time::Duration;

use db::DbMapper;
use memo::job::{JobDto, NewJobDto};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const LEASE_DURATION: Duration = Duration::from_secs(60);
const RENEW_INTERVAL: Duration = Duration::from_secs(20);
const RECOVERY_INTERVAL: Duration = Duration::from_secs(30);
const RETRY_DELAY_SECONDS: i64 = 30;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// Enqueue a job for later processing.
#[allow(dead_code)]
pub async fn enqueue(db: &DbMapper, job: NewJobDto) -> db::Result<JobDto> {
    db.jobs.enqueue(job).await
}

async fn logger_job_handler() {
    info!("TEST JOB");
}

async fn dispatch(job: &JobDto) -> std::result::Result<(), String> {
    match job.job_type.as_str() {
        "logger_job_handler" => {
            logger_job_handler().await;
            Ok(())
        }
        _ => Err("Unknown job type".to_string()),
    }
}

pub struct WorkerPool {
    shutdown: watch::Sender<bool>,
    supervisor: tokio::task::JoinHandle<()>,
}

impl WorkerPool {
    pub fn start(db: Arc<DbMapper>, count: usize) -> Self {
        let (shutdown, receiver) = watch::channel(false);
        let shutdown_receiver = receiver.clone();
        let mut tasks = JoinSet::new();
        for worker in 0..count {
            tasks.spawn(worker_loop(db.clone(), receiver.clone(), worker));
        }
        if count > 0 {
            tasks.spawn(recovery_loop(db, receiver));
            info!(count, "Job worker pool started");
        }
        let supervisor = tokio::spawn(async move {
            while let Some(result) = tasks.join_next().await {
                match result {
                    Err(error) => error!(%error, "Job worker task exited unexpectedly"),
                    Ok(()) if !*shutdown_receiver.borrow() => {
                        error!("Job worker task stopped unexpectedly");
                    }
                    Ok(()) => {}
                }
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

async fn wait_or_shutdown(shutdown: &mut watch::Receiver<bool>, delay: Duration) -> bool {
    if *shutdown.borrow() {
        return true;
    }
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        _ = shutdown.changed() => true,
    }
}

async fn worker_loop(db: Arc<DbMapper>, mut shutdown: watch::Receiver<bool>, worker: usize) {
    loop {
        if *shutdown.borrow() {
            break;
        }
        match db.jobs.claim_next(LEASE_DURATION).await {
            Ok(Some(job)) => {
                // A claim already in progress may finish after shutdown begins;
                // finish that job instead of abandoning its newly acquired lease.
                run_job(&db, job).await;
            }
            Ok(None) => {
                if wait_or_shutdown(&mut shutdown, POLL_INTERVAL).await {
                    break;
                }
            }
            Err(error) => {
                warn!(worker, %error, "Unable to claim job");
                if wait_or_shutdown(&mut shutdown, POLL_INTERVAL).await {
                    break;
                }
            }
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

async fn recovery_loop(db: Arc<DbMapper>, mut shutdown: watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow() {
            break;
        }
        if let Err(error) = db.jobs.recover_expired().await {
            warn!(%error, "Unable to recover expired jobs");
        }
        if wait_or_shutdown(&mut shutdown, RECOVERY_INTERVAL).await {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memo::job::JobStatus;

    #[tokio::test]
    async fn workers_execute_and_retry_jobs() {
        let dir =
            std::env::temp_dir().join(format!("memo-worker-test-{}", memo::utils::generate_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("jobs.db");
        let conn = db::db::create_db_pool(&path).await.unwrap();
        for statement in include_str!("../../db/db/migrations/memo/12-create-jobs.sql").split(';') {
            if !statement.trim().is_empty() {
                conn.execute(statement.trim(), ()).await.unwrap();
            }
        }
        drop(conn);
        let db = Arc::new(db::create_db_mapper(&path, 4).await.unwrap());
        let job = enqueue(
            &db,
            NewJobDto {
                job_type: "logger_job_handler".into(),
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
                if a.status == JobStatus::Completed && b.status == JobStatus::Failed {
                    assert_eq!(b.last_error.as_deref(), Some("Unknown job type"));
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
