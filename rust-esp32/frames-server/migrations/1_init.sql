CREATE TABLE IF NOT EXISTS frames
(
    id          INTEGER PRIMARY KEY NOT NULL,
    name        TEXT                NOT NULL,
    mac         TEXT                NOT NULL,
    temperature REAL                NOT NULL,
    payload     BLOB                NOT NULL,
    created_at  TEXT                DEFAULT CURRENT_TIMESTAMP
);
