CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    content TEXT NOT NULL,
    next_revision TEXT NOT NULL,
    checksum TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files(id)
) STRICT;

CREATE INDEX idx_notes_file_id ON notes(file_id);
CREATE UNIQUE INDEX idx_notes_file_id_next_revision ON notes(file_id, next_revision);
