CREATE TABLE IF NOT EXISTS frames
(
    id          INTEGER PRIMARY KEY NOT NULL,
    description TEXT                NOT NULL,
    done        BOOLEAN             NOT NULL DEFAULT 0
);
