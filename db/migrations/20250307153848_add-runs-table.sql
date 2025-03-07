create table classes
(
    id INTEGER primary key autoincrement,
    event_id INTEGER references events on delete cascade,
    name          TEXT,
    length        INTEGER,
    climb         INTEGER,
    control_count INTEGER,
    constraint classes_class_name unique (event_id, name)
);

create table runs
(
    id INTEGER primary key autoincrement,
    event_id INTEGER references events on delete cascade,
    run_id       integer,
    runner_name  text,
    class_name   text,
    si_id        integer,
    registration text,
    start_time   TEXT,
    check_time   TEXT,
    finish_time  TEXT,
    status       TEXT,
    constraint runs_runid_name unique (event_id, run_id)
);



