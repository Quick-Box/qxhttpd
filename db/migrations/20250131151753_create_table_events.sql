CREATE TABLE events
(
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    name  TEXT        NOT NULL,
    place TEXT        NOT NULL,
    date  DATETIME DEFAULT CURRENT_TIMESTAMP
);

create table ocout
(
    id INTEGER primary key,
    event_id INTEGER references events on delete cascade,
    change_set      TEXT
);

create table qein
(
    id INTEGER primary key,
    event_id INTEGER references events on delete cascade,
    original   TEXT,
    change   TEXT,
    source   TEXT,
    user_id  TEXT
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
