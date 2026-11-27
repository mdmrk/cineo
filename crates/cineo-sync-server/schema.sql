CREATE TABLE IF NOT EXISTS spaces (
    id BLOB PRIMARY KEY NOT NULL,
    seq INTEGER NOT NULL,
    records INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    touched_ms INTEGER NOT NULL,
    deleted INTEGER NOT NULL,
    lock INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS records (
    space BLOB NOT NULL,
    key BLOB NOT NULL,
    seq INTEGER NOT NULL,
    blob BLOB NOT NULL,
    PRIMARY KEY (space, key)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS records_by_seq ON records (space, seq);
