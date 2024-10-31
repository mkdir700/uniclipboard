-- Your SQL goes here
CREATE TABLE file_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    file_type TEXT NOT NULL,
    local_path TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX idx_file_metadata_code ON file_metadata (code);
CREATE INDEX idx_file_metadata_created_at ON file_metadata (created_at);

CREATE TRIGGER update_file_metadata_updated_at
AFTER UPDATE ON file_metadata
FOR EACH ROW
BEGIN
    UPDATE file_metadata SET updated_at = strftime('%s', 'now')
    WHERE id = NEW.id;
END;