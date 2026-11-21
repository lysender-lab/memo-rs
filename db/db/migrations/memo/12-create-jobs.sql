CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    job_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    status TEXT NOT NULL,
    attempts INTEGER NOT NULL,
    max_attempts INTEGER NOT NULL,
    available_at INTEGER NOT NULL,
    lease_token TEXT NOT NULL,
    lease_expires_at INTEGER NOT NULL,
    last_error TEXT NOT NULL,
    created_at INTEGER NOT NULL,
) STRICT;

CREATE INDEX idx_notes_file_id ON notes(file_id);
CREATE UNIQUE INDEX idx_notes_file_id_next_revision ON notes(file_id, next_revision);
