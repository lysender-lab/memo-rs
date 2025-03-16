CREATE TABLE clients (
    id CHAR(32) PRIMARY KEY NOT NULL,
    name VARCHAR(50) NOT NULL,
    status VARCHAR(10) NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX clients_name_idx ON clients(name);
