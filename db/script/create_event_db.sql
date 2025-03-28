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

create table occhng
(
    id INTEGER primary key autoincrement,
    change_set TEXT,
    created    TEXT default CURRENT_TIMESTAMP
);

create table qxchng
(
    id INTEGER primary key autoincrement,
    run_id INTEGER,
    property   TEXT,
    value   TEXT,
    status   TEXT,
    user_id  TEXT,
    created  TEXT default CURRENT_TIMESTAMP
);

create table runs
(
    id INTEGER primary key autoincrement,
    run_id       integer,
    class_name   text,
    first_name   text,
    last_name    TEXT,
    registration text,
    si_id        integer,
    start_time   TEXT,
    check_time   TEXT,
    finish_time  TEXT,
    status       TEXT,
    constraint runs_pk_2 unique (run_id)
);

