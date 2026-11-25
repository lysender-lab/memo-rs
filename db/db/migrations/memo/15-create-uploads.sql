CREATE TABLE uploads (
    id TEXT PRIMARY KEY,
    storage_key TEXT NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

CREATE INDEX idx_uploads_created_at
    ON uploads(created_at);
