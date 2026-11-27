use std::sync::Arc;
use std::time::Duration;

use memo::job::{JobDto, JobStatus, NewJobDto};
use memo::utils::{IdPrefix, generate_id, generate_prefixed_id};
use snafu::ResultExt;
use tokio::time::sleep;
use turso::{Connection, Row};

use crate::db_pool::DbPool;
use crate::error::{DbPrepareSnafu, DbStatementSnafu, ValidationSnafu};
use crate::turso_decode::{
    FromTursoRow, collect_row, opt_row_integer, opt_row_text, row_integer, row_text,
};
use crate::turso_params::{integer_param, text_param};
use crate::{Error, Result};

const COLUMNS: &str = "id, job_type, payload, status, attempts, max_attempts, available_at, \
    lease_token, lease_expires_at, last_error, created_at, updated_at";

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn lease_expiry(now: i64, duration: Duration) -> Result<i64> {
    let seconds = i64::try_from(duration.as_secs()).map_err(|_| crate::Error::Validation {
        msg: "Lease duration is too large".to_string(),
    })?;
    if seconds == 0 {
        return Err(crate::Error::Validation {
            msg: "Lease duration must be at least one second".to_string(),
        });
    }
    now.checked_add(seconds)
        .ok_or_else(|| crate::Error::Validation {
            msg: "Lease expiry is out of range".to_string(),
        })
}

impl FromTursoRow for JobDto {
    fn from_row(row: &Row) -> Result<Self> {
        let status = match row_text(row, 3)?.as_str() {
            "pending" => JobStatus::Pending,
            "running" => JobStatus::Running,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            status => return Err(format!("Invalid job status: {status}").into()),
        };
        Ok(Self {
            id: row_text(row, 0)?,
            job_type: row_text(row, 1)?,
            payload: row_text(row, 2)?,
            status,
            attempts: row_integer(row, 4)?,
            max_attempts: row_integer(row, 5)?,
            available_at: row_integer(row, 6)?,
            lease_token: opt_row_text(row, 7)?,
            lease_expires_at: opt_row_integer(row, 8)?,
            last_error: opt_row_text(row, 9)?,
            created_at: row_integer(row, 10)?,
            updated_at: row_integer(row, 11)?,
        })
    }
}

pub struct JobRepo {
    db_pool: Arc<DbPool>,
}

impl JobRepo {
    pub(crate) fn new(db_pool: Arc<DbPool>) -> Self {
        Self { db_pool }
    }

    /// Enqueue a new pending job.
    pub async fn enqueue(&self, input: NewJobDto) -> Result<JobDto> {
        let conn = self.db_pool.acquire().await?;
        Self::enqueue_on(&conn, input).await
    }

    pub(crate) async fn enqueue_on(conn: &Connection, input: NewJobDto) -> Result<JobDto> {
        if input.job_type.trim().is_empty() || input.max_attempts <= 0 {
            return ValidationSnafu {
                msg: "Job type must be nonempty and max_attempts must be positive".to_string(),
            }
            .fail();
        }
        if serde_json::from_str::<serde_json::Value>(&input.payload).is_err() {
            return ValidationSnafu {
                msg: "Job payload must be valid JSON".to_string(),
            }
            .fail();
        }
        let timestamp = now();
        let job = JobDto {
            id: generate_prefixed_id(IdPrefix::Job),
            job_type: input.job_type,
            payload: input.payload,
            status: JobStatus::Pending,
            attempts: 0,
            max_attempts: input.max_attempts,
            available_at: input.available_at,
            lease_token: None,
            lease_expires_at: None,
            last_error: None,
            created_at: timestamp,
            updated_at: timestamp,
        };
        let sql = "INSERT INTO jobs (id, job_type, payload, status, attempts, max_attempts, \
                   available_at, created_at, updated_at) \
                   VALUES (:id, :job_type, :payload, 'pending', 0, :max_attempts, \
                   :available_at, :now, :now)";
        let mut stmt = conn.prepare(sql).await.context(DbPrepareSnafu)?;
        stmt.execute(vec![
            text_param(":id", job.id.clone()),
            text_param(":job_type", job.job_type.clone()),
            text_param(":payload", job.payload.clone()),
            integer_param(":max_attempts", job.max_attempts),
            integer_param(":available_at", job.available_at),
            integer_param(":now", timestamp),
        ])
        .await
        .context(DbStatementSnafu)?;
        Ok(job)
    }

    /// Conditionally claim the next eligible job, or return None if none is available.
    pub async fn claim_next(&self, lease_duration: Duration) -> Result<Option<JobDto>> {
        let conn = self.db_pool.acquire().await?;
        // Validate even when there is no eligible job.
        lease_expiry(now(), lease_duration)?;
        let timestamp = now();
        let select = format!(
            "SELECT {COLUMNS} FROM jobs WHERE status = 'pending' \
             AND available_at <= :now AND attempts < max_attempts \
             ORDER BY available_at, created_at, id LIMIT 1"
        );
        let candidate: Option<JobDto> = {
            let mut stmt = conn.prepare(select).await.context(DbPrepareSnafu)?;
            collect_row(stmt.query_row(vec![integer_param(":now", timestamp)]).await)?
        };
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let timestamp = now();
        let expiry = lease_expiry(timestamp, lease_duration)?;
        let token = generate_id();
        let sql = "UPDATE jobs SET status = 'running', attempts = attempts + 1, \
             lease_token = :token, lease_expires_at = :expiry, updated_at = :now \
             WHERE id = :id AND status = 'pending' AND available_at <= :now \
             AND attempts < max_attempts";
        let affected = {
            let mut stmt = conn.prepare(sql).await.context(DbPrepareSnafu)?;
            stmt.execute(vec![
                text_param(":id", candidate.id.clone()),
                text_param(":token", token.clone()),
                integer_param(":expiry", expiry),
                integer_param(":now", timestamp),
            ])
            .await
            .context(DbStatementSnafu)?
        };
        if affected == 0 {
            return Ok(None);
        }
        let query = format!(
            "SELECT {COLUMNS} FROM jobs WHERE id = :id AND status = 'running' \
             AND lease_token = :token AND lease_expires_at > :now"
        );
        let mut stmt = conn.prepare(query).await.context(DbPrepareSnafu)?;
        let claimed = collect_row(
            stmt.query_row(vec![
                text_param(":id", candidate.id),
                text_param(":token", token),
                integer_param(":now", now()),
            ])
            .await,
        )?;
        match claimed {
            Some(job) => Ok(Some(job)),
            None => Err("Claimed job could not be retrieved with its unexpired lease".into()),
        }
    }

    /// Complete a job if the lease token still matches and has not expired.
    pub async fn complete(&self, job_id: &str, lease_token: &str) -> Result<bool> {
        let mut delay = Duration::from_millis(100);
        let mut attempt = 1;
        loop {
            match self.complete_once(job_id, lease_token).await {
                Err(Error::DbPrepare { ref source } | Error::DbStatement { ref source })
                    if attempt < 5
                        && matches!(
                            source,
                            turso::Error::Busy(_) | turso::Error::BusySnapshot(_)
                        ) =>
                {
                    sleep(delay).await;
                    delay *= 2;
                    attempt += 1;
                }
                result => return result,
            }
        }
    }

    async fn complete_once(&self, job_id: &str, lease_token: &str) -> Result<bool> {
        let conn = self.db_pool.acquire().await?;
        let sql = "UPDATE jobs SET status = 'completed', lease_token = NULL, \
                   lease_expires_at = NULL, updated_at = :now \
                   WHERE id = :id AND status = 'running' AND lease_token = :token \
                   AND lease_expires_at > :now";
        let mut stmt = conn.prepare(sql).await.context(DbPrepareSnafu)?;
        let affected = stmt
            .execute(vec![
                integer_param(":now", now()),
                text_param(":id", job_id.to_owned()),
                text_param(":token", lease_token.to_owned()),
            ])
            .await
            .context(DbStatementSnafu)?;
        Ok(affected > 0)
    }

    /// Retry at the specified Unix timestamp, or mark failed if attempts are exhausted.
    /// Returns false if the lease token no longer matches or has expired.
    pub async fn fail(
        &self,
        job_id: &str,
        lease_token: &str,
        error: &str,
        retry_at: i64,
    ) -> Result<bool> {
        let conn = self.db_pool.acquire().await?;
        let timestamp = now();
        let sql = "UPDATE jobs SET status = CASE WHEN attempts < max_attempts THEN 'pending' ELSE 'failed' END, \
                   available_at = CASE WHEN attempts < max_attempts THEN :retry_at ELSE available_at END, \
                   last_error = :error, lease_token = NULL, lease_expires_at = NULL, updated_at = :now \
                   WHERE id = :id AND status = 'running' AND lease_token = :token AND lease_expires_at > :now";
        let mut stmt = conn.prepare(sql).await.context(DbPrepareSnafu)?;
        let affected = stmt
            .execute(vec![
                integer_param(":retry_at", retry_at),
                text_param(":error", error.to_owned()),
                integer_param(":now", timestamp),
                text_param(":id", job_id.to_owned()),
                text_param(":token", lease_token.to_owned()),
            ])
            .await
            .context(DbStatementSnafu)?;
        Ok(affected > 0)
    }

    /// Extend an unexpired lease from the current time.
    pub async fn extend_lease(
        &self,
        job_id: &str,
        lease_token: &str,
        lease_duration: Duration,
    ) -> Result<bool> {
        let conn = self.db_pool.acquire().await?;
        let timestamp = now();
        let expiry = lease_expiry(timestamp, lease_duration)?;
        let sql = "UPDATE jobs SET lease_expires_at = :expiry, updated_at = :now \
                   WHERE id = :id AND status = 'running' AND lease_token = :token AND lease_expires_at > :now";
        let mut stmt = conn.prepare(sql).await.context(DbPrepareSnafu)?;
        let affected = stmt
            .execute(vec![
                integer_param(":expiry", expiry),
                integer_param(":now", timestamp),
                text_param(":id", job_id.to_owned()),
                text_param(":token", lease_token.to_owned()),
            ])
            .await
            .context(DbStatementSnafu)?;
        Ok(affected > 0)
    }

    /// Recover expired leases, returning the number of jobs changed.
    pub async fn recover_expired(&self) -> Result<u64> {
        let conn = self.db_pool.acquire().await?;
        let timestamp = now();
        let sql = "UPDATE jobs SET status = CASE WHEN attempts < max_attempts THEN 'pending' ELSE 'failed' END, \
                   available_at = CASE WHEN attempts < max_attempts THEN :now ELSE available_at END, \
                   lease_token = NULL, lease_expires_at = NULL, updated_at = :now \
                   WHERE status = 'running' AND lease_expires_at <= :now";
        let mut stmt = conn.prepare(sql).await.context(DbPrepareSnafu)?;
        let affected = stmt
            .execute(vec![integer_param(":now", timestamp)])
            .await
            .context(DbStatementSnafu)?;
        Ok(affected)
    }

    /// Delete completed and failed jobs updated before the Unix timestamp cutoff.
    pub async fn delete_finished_before(&self, cutoff: i64) -> Result<u64> {
        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn
            .prepare(
                "DELETE FROM jobs WHERE status IN ('completed', 'failed') AND updated_at < :cutoff",
            )
            .await
            .context(DbPrepareSnafu)?;
        let affected = stmt
            .execute(vec![integer_param(":cutoff", cutoff)])
            .await
            .context(DbStatementSnafu)?;
        Ok(affected)
    }

    /// Get a job by ID.
    pub async fn get(&self, job_id: &str) -> Result<Option<JobDto>> {
        let conn = self.db_pool.acquire().await?;
        let mut stmt = conn
            .prepare(format!("SELECT {COLUMNS} FROM jobs WHERE id = :id"))
            .await
            .context(DbPrepareSnafu)?;
        collect_row(
            stmt.query_row(vec![text_param(":id", job_id.to_owned())])
                .await,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::TestCtx;
    use turso::Value;

    fn input(max_attempts: i64, available_at: i64) -> NewJobDto {
        NewJobDto {
            job_type: "example".to_string(),
            payload: "{}".to_string(),
            max_attempts,
            available_at,
        }
    }

    async fn expire(ctx: &TestCtx, id: &str) {
        ctx.db
            .any
            .execute(
                "UPDATE jobs SET lease_expires_at = :expiry WHERE id = :id".to_string(),
                vec![
                    (":expiry".into(), Value::Integer(now())),
                    (":id".into(), Value::Text(id.into())),
                ],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn enqueue_validation_and_round_trip() {
        let ctx = TestCtx::new("jobs-enqueue").await.unwrap();
        let repo = &ctx.db.jobs;
        assert!(repo.enqueue(input(0, now())).await.is_err());
        let mut bad = input(1, now());
        bad.payload = "not json".into();
        assert!(repo.enqueue(bad).await.is_err());
        let mut bad = input(1, now());
        bad.job_type = " ".into();
        assert!(repo.enqueue(bad).await.is_err());
        let job = repo.enqueue(input(3, now())).await.unwrap();
        assert!(job.id.starts_with("job_"));
        assert!(memo::utils::valid_prefixed_id(&job.id));
        let stored = repo.get(&job.id).await.unwrap().unwrap();
        assert_eq!(stored.id, job.id);
        assert_eq!(stored.status, JobStatus::Pending);
        assert_eq!(stored.attempts, 0);
        assert!(
            stored.lease_token.is_none()
                && stored.lease_expires_at.is_none()
                && stored.last_error.is_none()
        );
        assert!(repo.get("missing").await.unwrap().is_none());
        ctx.db
            .any
            .execute(
                "UPDATE jobs SET status = 'invalid' WHERE id = :id".into(),
                vec![(":id".into(), Value::Text(job.id))],
            )
            .await
            .unwrap();
        assert!(repo.get(&stored.id).await.is_err());
    }

    #[tokio::test]
    async fn empty_polls_and_recovery_allow_later_claim() {
        let ctx = TestCtx::new("jobs-empty-polls").await.unwrap();
        let repo = &ctx.db.jobs;
        for _ in 0..20 {
            assert!(
                repo.claim_next(Duration::from_secs(30))
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(repo.recover_expired().await.unwrap(), 0);
        }
        let inserted = repo.enqueue(input(2, now())).await.unwrap();
        let claimed = repo
            .claim_next(Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, inserted.id);
        assert_eq!(claimed.status, JobStatus::Running);
        assert_eq!(claimed.attempts, 1);
        assert!(claimed.lease_token.is_some());
    }

    #[tokio::test]
    async fn claim_retry_complete_and_cleanup() {
        let ctx = TestCtx::new("jobs-lifecycle").await.unwrap();
        let repo = &ctx.db.jobs;
        let future = repo.enqueue(input(2, now() + 3600)).await.unwrap();
        assert!(
            repo.claim_next(Duration::from_secs(30))
                .await
                .unwrap()
                .is_none()
        );
        let job = repo.enqueue(input(2, now() - 1)).await.unwrap();
        let claimed = repo
            .claim_next(Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, job.id);
        assert_eq!(claimed.attempts, 1);
        let token = claimed.lease_token.unwrap();
        assert!(!repo.complete(&job.id, "wrong").await.unwrap());
        assert!(
            !repo
                .extend_lease(&job.id, "wrong", Duration::from_secs(30))
                .await
                .unwrap()
        );
        assert!(
            repo.extend_lease(&job.id, &token, Duration::from_secs(60))
                .await
                .unwrap()
        );
        assert!(
            repo.fail(&job.id, &token, "failure", now() - 1)
                .await
                .unwrap()
        );
        let retried = repo
            .claim_next(Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retried.attempts, 2);
        let new_token = retried.lease_token.unwrap();
        assert_ne!(token, new_token);
        assert!(!repo.complete(&job.id, &token).await.unwrap());
        assert!(repo.complete(&job.id, &new_token).await.unwrap());
        let completed = repo.get(&job.id).await.unwrap().unwrap();
        assert_eq!(completed.status, JobStatus::Completed);
        assert_eq!(completed.last_error.as_deref(), Some("failure"));
        assert_eq!(
            repo.delete_finished_before(completed.updated_at)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            repo.delete_finished_before(completed.updated_at + 1)
                .await
                .unwrap(),
            1
        );
        assert!(repo.get(&future.id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn complete_retries_write_contention() -> Result<()> {
        let ctx = TestCtx::new("jobs-complete-contention").await?;
        let repo = &ctx.db.jobs;
        repo.enqueue(input(2, now())).await?;
        let job = repo.claim_next(Duration::from_secs(60)).await?.unwrap();
        let token = job.lease_token.as_deref().unwrap();
        let blocker = repo.db_pool.acquire().await?;
        blocker
            .execute("BEGIN IMMEDIATE", ())
            .await
            .context(DbStatementSnafu)?;

        assert!(matches!(
            repo.complete(&job.id, token).await,
            Err(Error::DbPrepare { source } | Error::DbStatement { source })
                if matches!(source, turso::Error::Busy(_) | turso::Error::BusySnapshot(_))
        ));
        assert_eq!(repo.get(&job.id).await?.unwrap().status, JobStatus::Running);

        let completion = repo.complete(&job.id, token);
        tokio::pin!(completion);
        tokio::select! {
            biased;
            result = &mut completion => panic!("completion returned while write was blocked: {result:?}"),
            _ = sleep(Duration::from_millis(50)) => {}
        }
        blocker
            .execute("ROLLBACK", ())
            .await
            .context(DbStatementSnafu)?;

        assert!(completion.await?);
        let completed = repo.get(&job.id).await?.unwrap();
        assert_eq!(completed.status, JobStatus::Completed);
        assert_eq!(completed.attempts, job.attempts);
        assert!(completed.lease_token.is_none() && completed.lease_expires_at.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn complete_rechecks_lease_after_write_contention() -> Result<()> {
        let ctx = TestCtx::new("jobs-complete-expiry").await?;
        let repo = &ctx.db.jobs;
        repo.enqueue(input(2, now())).await?;
        let job = repo.claim_next(Duration::from_secs(60)).await?.unwrap();
        let token = job.lease_token.as_deref().unwrap();
        let blocker = repo.db_pool.acquire().await?;
        blocker
            .execute("BEGIN IMMEDIATE", ())
            .await
            .context(DbStatementSnafu)?;

        let completion = repo.complete(&job.id, token);
        tokio::pin!(completion);
        tokio::select! {
            biased;
            result = &mut completion => panic!("completion returned while write was blocked: {result:?}"),
            _ = sleep(Duration::from_millis(50)) => {}
        }
        blocker
            .execute(
                "UPDATE jobs SET lease_expires_at = :expiry WHERE id = :id",
                vec![
                    integer_param(":expiry", now()),
                    text_param(":id", job.id.clone()),
                ],
            )
            .await
            .context(DbStatementSnafu)?;
        blocker
            .execute("COMMIT", ())
            .await
            .context(DbStatementSnafu)?;

        assert!(!completion.await?);
        assert_eq!(repo.get(&job.id).await?.unwrap().status, JobStatus::Running);
        Ok(())
    }

    #[tokio::test]
    async fn cleanup_only_deletes_terminal_jobs_before_cutoff() {
        let ctx = TestCtx::new("jobs-cleanup-retention").await.unwrap();
        let repo = &ctx.db.jobs;
        let cutoff = now() - 7 * 24 * 60 * 60;
        let cases = [
            ("completed", cutoff - 1, true),
            ("failed", cutoff - 1, true),
            ("completed", cutoff, false),
            ("failed", cutoff + 1, false),
            ("pending", cutoff - 1, false),
            ("running", cutoff - 1, false),
        ];
        let mut jobs = Vec::new();
        for (status, updated_at, deleted) in cases {
            let job = repo.enqueue(input(1, now())).await.unwrap();
            ctx.db
                .any
                .execute(
                    "UPDATE jobs SET status = :status, updated_at = :updated_at WHERE id = :id"
                        .into(),
                    vec![
                        (":status".into(), Value::Text(status.into())),
                        (":updated_at".into(), Value::Integer(updated_at)),
                        (":id".into(), Value::Text(job.id.clone())),
                    ],
                )
                .await
                .unwrap();
            jobs.push((job.id, deleted));
        }

        assert_eq!(repo.delete_finished_before(cutoff).await.unwrap(), 2);
        for (id, deleted) in jobs {
            assert_eq!(repo.get(&id).await.unwrap().is_none(), deleted);
        }
    }

    #[tokio::test]
    async fn expired_lease_recovery_and_exhaustion() {
        let ctx = TestCtx::new("jobs-expiry").await.unwrap();
        let repo = &ctx.db.jobs;
        let job = repo.enqueue(input(2, now())).await.unwrap();
        let first = repo
            .claim_next(Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let token = first.lease_token.unwrap();
        assert!(
            repo.fail(&job.id, &token, "previous error", now() - 1)
                .await
                .unwrap()
        );
        let second = repo
            .claim_next(Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let token = second.lease_token.unwrap();
        expire(&ctx, &job.id).await;
        assert!(!repo.complete(&job.id, &token).await.unwrap());
        assert!(!repo.fail(&job.id, &token, "late", now()).await.unwrap());
        assert!(
            !repo
                .extend_lease(&job.id, &token, Duration::from_secs(30))
                .await
                .unwrap()
        );
        assert_eq!(repo.recover_expired().await.unwrap(), 1);
        let failed = repo.get(&job.id).await.unwrap().unwrap();
        assert_eq!(failed.status, JobStatus::Failed);
        assert_eq!(failed.last_error.as_deref(), Some("previous error"));
        assert!(failed.lease_token.is_none() && failed.lease_expires_at.is_none());

        let job = repo.enqueue(input(2, now())).await.unwrap();
        repo.claim_next(Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        expire(&ctx, &job.id).await;
        assert_eq!(repo.recover_expired().await.unwrap(), 1);
        assert_eq!(
            repo.get(&job.id).await.unwrap().unwrap().status,
            JobStatus::Pending
        );
    }

    #[tokio::test]
    async fn concurrent_claims_and_invalid_lease() {
        let ctx = TestCtx::new("jobs-concurrent").await.unwrap();
        let repo = &ctx.db.jobs;
        assert!(repo.claim_next(Duration::ZERO).await.is_err());
        assert!(repo.claim_next(Duration::MAX).await.is_err());
        repo.enqueue(input(2, now())).await.unwrap();
        let (a, b) = tokio::join!(
            repo.claim_next(Duration::from_secs(30)),
            repo.claim_next(Duration::from_secs(30)),
        );
        // A concurrent writer may receive a Turso busy/conflict error; it must
        // never receive a second successful claim for the same attempt.
        let claimed = [a, b]
            .into_iter()
            .filter(|r| matches!(r, Ok(Some(_))))
            .count();
        assert_eq!(claimed, 1);
        assert!(
            repo.claim_next(Duration::from_secs(30))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn scheduling_and_terminal_failure() {
        let ctx = TestCtx::new("jobs-scheduling").await.unwrap();
        let repo = &ctx.db.jobs;
        let later = repo.enqueue(input(1, now() - 1)).await.unwrap();
        let earlier = repo.enqueue(input(1, now() - 10)).await.unwrap();
        let claimed = repo
            .claim_next(Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, earlier.id);
        let token = claimed.lease_token.unwrap();
        assert!(
            repo.fail(&earlier.id, &token, "terminal", now() + 100)
                .await
                .unwrap()
        );
        let failed = repo.get(&earlier.id).await.unwrap().unwrap();
        assert_eq!(failed.status, JobStatus::Failed);
        assert_eq!(failed.last_error.as_deref(), Some("terminal"));
        assert!(failed.lease_token.is_none());
        assert_eq!(
            repo.claim_next(Duration::from_secs(30))
                .await
                .unwrap()
                .unwrap()
                .id,
            later.id
        );
        assert_eq!(repo.delete_finished_before(now() + 1).await.unwrap(), 1);
        assert!(repo.get(&later.id).await.unwrap().is_some());
    }
}
