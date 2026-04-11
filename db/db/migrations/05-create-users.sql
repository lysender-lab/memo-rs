CREATE TABLE users (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    username TEXT NOT NULL,
    password TEXT NOT NULL,
    status TEXT NOT NULL,
    roles TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (client_id) REFERENCES clients(id)
) STRICT;

CREATE INDEX idx_users_client_id ON users(client_id);
CREATE UNIQUE INDEX idx_users_username ON users(username);
