create table events
(
    id         INTEGER
        primary key autoincrement,
    name       TEXT not null,
    place      TEXT not null,
    start_time TEXT default CURRENT_TIMESTAMP,
    api_token  TEXT,
    owner      TEXT
);

create unique index events_api_token_uindex
    on events (api_token);

create table files
(
    id        INTEGER
        primary key,
    event_id  INTEGER
        references events
            on delete cascade,
    file_name TEXT not null,
    data      BLOB not null,
    created   TEXT default CURRENT_TIMESTAMP
);

create unique index files_file_name_index
    on files (event_id, file_name);

create table ocout
(
    id         INTEGER
        primary key,
    event_id   INTEGER
        references events
            on delete cascade,
    change_set TEXT,
    created    TEXT default CURRENT_TIMESTAMP
);

create table qein
(
    id       INTEGER
        primary key,
    event_id INTEGER
        references events
            on delete cascade,
    original TEXT,
    change   TEXT,
    source   TEXT,
    user_id  TEXT,
    created  TEXT default CURRENT_TIMESTAMP
);

create table qeout
(
    id       INTEGER
        primary key,
    event_id INTEGER
        references events
            on delete cascade,
    change   TEXT,
    source   TEXT,
    user_id  TEXT,
    created  TEXT default CURRENT_TIMESTAMP
);

