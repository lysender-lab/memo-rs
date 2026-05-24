CREATE TABLE dirs (
    id TEXT PRIMARY KEY,
    bucket_id TEXT NOT NULL,
    name TEXT NOT NULL,
    label TEXT NOT NULL,
    file_count INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER NULL DEFAULT NULL,
    FOREIGN KEY (bucket_id) REFERENCES buckets(id)
) STRICT;

CREATE INDEX idx_dirs_bucket_id ON dirs(bucket_id);
CREATE UNIQUE INDEX idx_dirs_bucket_id_name_deleted_at ON dirs(bucket_id, name, deleted_at);
CREATE UNIQUE INDEX idx_dirs_bucket_id_label_deleted_at ON dirs(bucket_id, label, deleted_at);
CREATE INDEX idx_dirs_bucket_id_updated_at_deleted_at ON dirs(bucket_id, updated_at, deleted_at);
