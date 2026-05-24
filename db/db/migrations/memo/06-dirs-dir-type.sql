PRAGMA foreign_keys = OFF;

BEGIN TRANSACTION;

CREATE TABLE dirs_new (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL,
    dir_type TEXT NOT NULL,
    name TEXT NOT NULL,
    label TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER NULL DEFAULT NULL
) STRICT;

INSERT INTO dirs_new (
  id, org_id, dir_type, name, label, created_at, updated_at, deleted_at
)
SELECT
  id, bucket_id, "photos", name, label, created_at, updated_at, deleted_at
FROM dirs;

DROP TABLE dirs;

ALTER TABLE dirs_new RENAME TO dirs;

CREATE INDEX idx_dirs_org_dir_type ON dirs(org_id, dir_type, deleted_at);
CREATE UNIQUE INDEX idx_dirs_org_dir_type_name_deleted_at ON dirs(org_id, dir_type, name, deleted_at);
CREATE UNIQUE INDEX idx_dirs_org_dir_type_label_deleted_at ON dirs(org_id, dir_type, label, deleted_at);
CREATE INDEX idx_dirs_org_dir_type_updated_at_deleted_at ON dirs(org_id, dir_type, updated_at, deleted_at);

COMMIT;

PRAGMA foreign_keys = ON;
