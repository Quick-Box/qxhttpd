CREATE TABLE events
(
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    name  TEXT        NOT NULL,
    place TEXT UNIQUE NOT NULL,
    date  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE ocout
(
    id           INTEGER PRIMARY KEY,
    event_id     INTEGER,
    runner_id    INTEGER,
    start_status TEXT,
    si_id        INTEGER,
    class_name   TEXT,
    runner_name  TEXT,
    start_time   TEXT,
    comment      TEXT,
    FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE
);

CREATE TABLE qein
(
    id         INTEGER PRIMARY KEY,
    event_id   INTEGER,
    si_id      INTEGER,
    check_time TEXT,
    comment    TEXT,
    FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE
);

CREATE TABLE qeout
(
    id          INTEGER PRIMARY KEY,
    event_id    INTEGER,
    run_id      INTEGER,
    si_id       INTEGER,
    start_time  TEXT,
    finish_time TEXT,
    comment     TEXT,
    FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE
);
