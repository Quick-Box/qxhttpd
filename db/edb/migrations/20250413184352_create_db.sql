create table changes
(
    id             INTEGER primary key autoincrement,
    source         TEXT not null,
    data_type      TEXT not null,
    data_id        INTEGER,
    data           TEXT,
    user_id        TEXT,
    created        TEXT default CURRENT_TIMESTAMP,
    status         TEXT,
    status_message TEXT,
    lock_number    INTEGER
);

create table classes
(
    id INTEGER primary key autoincrement,
    name TEXT constraint classes_class_name unique,
    length           INTEGER,
    climb            INTEGER,
    control_count    INTEGER,
    start_time       INTEGER,
    interval         INTEGER,
    start_slot_count INTEGER
);

create table files
(
    id INTEGER primary key autoincrement,
    name TEXT not null constraint files_file_name_index unique,
    data    BLOB not null,
    created TEXT default CURRENT_TIMESTAMP
);

create table runs
(
    run_id INTEGER not null constraint runs_run_id primary key,
    class_name   TEXT,
    first_name   TEXT,
    last_name    TEXT,
    registration TEXT,
    si_id        INTEGER,
    start_time   TEXT,
    check_time   TEXT,
    finish_time  TEXT,
    status       TEXT
);

