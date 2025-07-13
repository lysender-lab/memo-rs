CREATE TABLE users (
    id CHAR(32) PRIMARY KEY NOT NULL,
    client_id CHAR(32) NOT NULL,
    username VARCHAR(30) NOT NULL,
    password VARCHAR(250) NOT NULL,
    status VARCHAR(10) NOT NULL,
    roles VARCHAR(250) NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (client_id) REFERENCES clients(id)
);
CREATE INDEX users_client_id_idx ON users(client_id);
CREATE UNIQUE INDEX users_username_idx ON users(username);
