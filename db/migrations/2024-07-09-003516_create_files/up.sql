CREATE TABLE files (
    id CHAR(32) PRIMARY KEY NOT NULL,
    dir_id CHAR(32) NOT NULL,
    name VARCHAR(250) NOT NULL,
    filename VARCHAR(250) NOT NULL,
    content_type VARCHAR(250) NOT NULL,
    size BIGINT NOT NULL,
    is_image INTEGER NOT NULL,
    img_dimension VARCHAR(100) NULL,
    img_versions VARCHAR(100) NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (dir_id) REFERENCES files(id)
);
CREATE INDEX files_dir_id_idx ON files(dir_id);
CREATE UNIQUE INDEX files_dir_id_name_idx ON files(dir_id, name);
CREATE INDEX files_dir_id_created_at_idx ON files(dir_id, created_at);
