CREATE TABLE clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    default_bucket_id TEXT NULL DEFAULT NULL,
    admin INTEGER NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NULL DEFAULT NULL,
    deleted_at INTEGER NULL DEFAULT NULL
) STRICT;

CREATE UNIQUE INDEX idx_clients_name_deleted_at ON clients(name, deleted_at);
