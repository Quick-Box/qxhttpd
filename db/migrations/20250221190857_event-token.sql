alter table events add api_token TEXT;
create index ocout_event_id_index on ocout (event_id);
create index qein_event_id_index on qein (event_id);
create index qeout_event_id_index on qeout (event_id);
