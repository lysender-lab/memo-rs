CREATE TABLE clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    default_bucket_id TEXT NULL DEFAULT NULL,
    admin INTEGER NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
) STRICT;

CREATE UNIQUE INDEX idx_clients_name ON clients(name);
