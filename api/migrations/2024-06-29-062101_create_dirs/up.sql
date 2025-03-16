CREATE TABLE dirs (
    id CHAR(32) PRIMARY KEY NOT NULL,
    bucket_id CHAR(32) NOT NULL,
    name VARCHAR(50) NOT NULL,
    label VARCHAR(60) NOT NULL,
    file_count INTEGER NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (bucket_id) REFERENCES buckets(id)
);
CREATE INDEX dirs_bucket_id_idx ON dirs(bucket_id);
CREATE UNIQUE INDEX dirs_bucket_id_name_idx ON dirs(bucket_id, name);
CREATE UNIQUE INDEX dirs_bucket_id_label_idx ON dirs(bucket_id, label);
CREATE INDEX dirs_bucket_id_updated_at_idx ON dirs(bucket_id, updated_at);
