CREATE TABLE files (
    id TEXT PRIMARY KEY,
    dir_id TEXT NOT NULL,
    name TEXT NOT NULL,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    is_image INTEGER NOT NULL,
    img_dimension TEXT NOT NULL,
    img_versions TEXT NOT NULL,
    img_taken_at INTEGER NULL DEFAULT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (dir_id) REFERENCES dirs(id)
) STRICT;

CREATE INDEX idx_files_dir_id ON files(dir_id);
CREATE UNIQUE INDEX idx_files_dir_id_name ON files(dir_id, name);
CREATE INDEX idx_files_dir_id_created_at ON files(dir_id, created_at);
