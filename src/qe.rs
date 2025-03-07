use chrono::NaiveDateTime;
use rocket::State;
use crate::db::DbPool;
use crate::event::{EventId, START_LIST_IOFXML3_FILE};
use crate::iofxml3;
use crate::iofxml3::structs::StartList;
use crate::util::{parse_naive_datetime, tee_sqlx_error};

mod runs;
mod classes;

fn start00(stlist: &StartList) -> Option<NaiveDateTime> {
    let d = stlist.event.start_time.date.as_str();
    let t = stlist.event.start_time.time.as_str();
    parse_naive_datetime(&format!("{d}T{t}"))    
}
pub async fn parse_startlist_event(event_id: EventId, db: &State<DbPool>) -> anyhow::Result<()> {
    let data = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND name=?")
        .bind(event_id)
        .bind(START_LIST_IOFXML3_FILE)
        .fetch_one(&db.0)
        .await.map_err(tee_sqlx_error)?.0;
    parse_startlist_xml_data(event_id, data, db).await
}
pub async fn parse_startlist_xml_data(event_id: EventId, data: Vec<u8>, db: &State<DbPool>) -> anyhow::Result<()> {
    let stlist = iofxml3::parser::parse_startlist(&data)?;
    let start00 = start00(&stlist).ok_or(anyhow::anyhow!("Invalid start list date time"))?;
    for cs in &stlist.class_start {
        let class_name = cs.class.name.as_str();
        for ps in &cs.person_start {
            let Some(run_id) = ps.person.id.iter().find(|id| id.id_type == "QuickEvent") else {
                warn!("QuickEvent ID not found in person_start {:?}", ps);
                continue;
            };
        }
    }
    Ok(())
}