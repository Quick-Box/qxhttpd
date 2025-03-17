create table events
(
    id INTEGER primary key autoincrement,
    name       TEXT not null,
    place      TEXT not null,
    start_time TEXT default CURRENT_TIMESTAMP,
    api_token  TEXT,
    owner      TEXT,
    constraint events_api_token_uindex unique (api_token)
);

create table classes
(
    id INTEGER primary key autoincrement,
    name             TEXT,
    length           INTEGER,
    climb            INTEGER,
    control_count    INTEGER,
    event_id INTEGER references events(id) on delete cascade,
    start_time       TEXT,
    interval         INTEGER,
    start_slot_count INTEGER,
    constraint classes_class_name unique (event_id, name)
);
CREATE INDEX idx_classes_event_id ON classes (event_id);

create table files
(
    id INTEGER primary key autoincrement,
    event_id INTEGER references events(id) on delete cascade,
    name     TEXT not null,
    data     BLOB not null,
    created  TEXT default CURRENT_TIMESTAMP,
    constraint files_file_name_index unique (event_id, name)
);
CREATE INDEX idx_files_event_id ON classes (event_id);

create table ocout
(
    id INTEGER primary key autoincrement,
    event_id INTEGER references events(id) on delete cascade,
    change_set TEXT,
    created    TEXT default CURRENT_TIMESTAMP
);
CREATE INDEX idx_ocout_event_id ON classes (event_id);

create table qein
(
    id INTEGER primary key autoincrement,
    event_id INTEGER references events(id) on delete cascade,
    original TEXT,
    change   TEXT,
    source   TEXT,
    user_id  TEXT,
    created  TEXT default CURRENT_TIMESTAMP
);
CREATE INDEX idx_qein_event_id ON classes (event_id);

create table qeout
(
    id INTEGER primary key autoincrement,
    event_id INTEGER references events(id) on delete cascade,
    change   TEXT,
    source   TEXT,
    user_id  TEXT,
    created  TEXT default CURRENT_TIMESTAMP
);
CREATE INDEX idx_qeout_event_id ON classes (event_id);

create table runs
(
    id INTEGER primary key autoincrement,
    event_id INTEGER references events(id) on delete cascade,
    run_id       integer,
    first_name   text,
    class_name   text,
    si_id        integer,
    registration text,
    start_time   TEXT,
    check_time   TEXT,
    finish_time  TEXT,
    status       TEXT,
    edited_by    TEXT,
    last_name    TEXT,
    constraint runs_pk_2 unique (event_id, run_id)
);
CREATE INDEX idx_runs_event_id ON classes (event_id);

