CREATE TABLE buckets (
    id CHAR(32) PRIMARY KEY NOT NULL,
    client_id CHAR(32) NOT NULL,
    name VARCHAR(50) NOT NULL,
    images_only INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    FOREIGN KEY (client_id) REFERENCES clients(id)
);
CREATE INDEX buckets_client_id_idx ON buckets(client_id);
CREATE UNIQUE INDEX buckets_client_id_name_idx ON buckets(client_id, name);
