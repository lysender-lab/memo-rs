CREATE TABLE uploads (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL,
    dir_id TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size INTEGER NOT NULL CHECK (size >= 0),
    storage_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'cleaning')),
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    CHECK (expires_at > created_at)
) STRICT;

CREATE INDEX idx_uploads_cleanup
    ON uploads(status, expires_at);
