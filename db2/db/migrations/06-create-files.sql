CREATE TABLE files (
    id TEXT PRIMARY KEY,
    dir_id TEXT NOT NULL,
    name TEXT NOT NULL,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    is_image INTEGER NOT NULL,
    img_dimension TEXT NULL DEFAULT NULL,
    img_versions TEXT NULL DEFAULT NULL,
    img_taken_at INTEGER NULL DEFAULT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER NULL DEFAULT NULL,
    FOREIGN KEY (dir_id) REFERENCES dirs(id)
) STRICT;

CREATE INDEX idx_files_dir_id ON files(dir_id);
CREATE UNIQUE INDEX idx_files_dir_id_name_deleted_at ON files(dir_id, name, deleted_at);
CREATE INDEX idx_files_dir_id_created_at_deleted_at ON files(dir_id, created_at, deleted_at);
