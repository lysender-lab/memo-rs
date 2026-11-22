CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    job_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    status TEXT NOT NULL,
    attempts INTEGER NOT NULL,
    max_attempts INTEGER NOT NULL,
    available_at INTEGER NOT NULL,
    lease_token TEXT NULL DEFAULT NULL,
    lease_expires_at INTEGER NULL DEFAULT NULL,
    last_error TEXT NULL DEFAULT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

CREATE INDEX idx_jobs_pending
    ON jobs(status, available_at, created_at, id);

CREATE INDEX idx_jobs_expired_leases
    ON jobs(status, lease_expires_at);
