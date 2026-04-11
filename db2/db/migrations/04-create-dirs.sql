CREATE TABLE dirs (
    id TEXT PRIMARY KEY,
    bucket_id TEXT NOT NULL,
    name TEXT NOT NULL,
    label TEXT NOT NULL,
    file_count INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (bucket_id) REFERENCES buckets(id)
) STRICT;

CREATE INDEX idx_dirs_bucket_id ON dirs(bucket_id);
CREATE UNIQUE INDEX idx_dirs_bucket_id_name ON dirs(bucket_id, name);
CREATE UNIQUE INDEX idx_dirs_bucket_id_label ON dirs(bucket_id, label);
CREATE INDEX idx_dirs_bucket_id_updated_at ON dirs(bucket_id, updated_at);
