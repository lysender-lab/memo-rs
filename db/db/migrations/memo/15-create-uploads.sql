CREATE TABLE uploads (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL,
    dir_id TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    storage_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX idx_uploads_storage_key
    ON uploads(storage_key);

CREATE INDEX idx_uploads_cleanup
    ON uploads(status, expires_at);
