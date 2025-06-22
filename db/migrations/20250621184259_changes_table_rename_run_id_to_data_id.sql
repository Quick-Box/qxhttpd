create table changes_dg_tmp
(
    id             INTEGER primary key autoincrement,
    source         TEXT not null,
    data_type      TEXT not null,
    data_id        INTEGER,
    data           TEXT,
    user_id        TEXT,
    created        TEXT default CURRENT_TIMESTAMP,
    status         TEXT,
    status_message TEXT
);

insert into changes_dg_tmp(id, source, data_type, data, data_id, status, user_id, created, status_message)
select id,
       source,
       data_type,
       data,
       run_id,
       status,
       user_id,
       created,
       status_message
from changes;

drop table changes;

alter table changes_dg_tmp
    rename to changes;

