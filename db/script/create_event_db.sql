create table classes
(
    id INTEGER primary key autoincrement,
    name             TEXT,
    length           INTEGER,
    climb            INTEGER,
    control_count    INTEGER,
    start_time       TEXT,
    interval         INTEGER,
    start_slot_count INTEGER,
    constraint classes_class_name unique (name)
);

create table files
(
    id INTEGER primary key autoincrement,
    name     TEXT not null,
    data     BLOB not null,
    created  TEXT default CURRENT_TIMESTAMP,
    constraint files_file_name_index unique (name)
);

create table changes
(
    id INTEGER primary key autoincrement,
    source   TEXT not null,
    data_type   TEXT not null,
    data   TEXT,
    run_id INTEGER,
    status   TEXT,
    user_id  TEXT,
    created  TEXT default CURRENT_TIMESTAMP
);

create table runs
(
    run_id       INTEGER not null primary key,
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

