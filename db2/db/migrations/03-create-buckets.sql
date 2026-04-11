CREATE TABLE buckets (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    name TEXT NOT NULL,
    label TEXT NOT NULL,
    images_only INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (client_id) REFERENCES clients(id)
) STRICT;

CREATE INDEX idx_buckets_client_id ON buckets(client_id);
CREATE UNIQUE INDEX idx_buckets_client_id_name ON buckets(client_id, name);
