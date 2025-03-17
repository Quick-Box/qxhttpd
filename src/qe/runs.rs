use anyhow::anyhow;
use chrono::{DateTime, FixedOffset};
use rocket::State;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Database, FromRow, Sqlite};
use sqlx::query::Query;
use crate::db::DbPool;
use crate::qe::{QEJournalRecord};
use crate::util::sqlx_to_anyhow;

#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct RunsRecord {
    pub id: i64,
    pub event_id: i64,
    pub run_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub class_name: String,
    pub si_id: i64,
    pub registration: String,
    pub start_time: Option<DateTime<FixedOffset>>,
    pub check_time: Option<DateTime<FixedOffset>>,
    pub finish_time: Option<DateTime<FixedOffset>>,
    pub status: String,
    pub edited_by: String,
}
impl Default for RunsRecord {
    fn default() -> Self {
        Self {
            id: 0,
            event_id: 0,
            run_id: 0,
            first_name: "".to_string(),
            last_name: "".to_string(),
            class_name: "".to_string(),
            si_id: 0,
            registration: "".to_string(),
            start_time: Default::default(),
            check_time: Default::default(),
            finish_time: Default::default(),
            status: "".to_string(),
            edited_by: "".to_string(),
        }
    }
}

pub async fn apply_qe_out_change(rec: &QEJournalRecord, db: &State<DbPool>) -> anyhow::Result<()> {
    let event_id = if rec.event_id > 0 { rec.event_id } else { 
        return Err(anyhow!("Event ID must not be 0."));
    };
    let run_id = rec.change.run_id.ok_or(anyhow!("Run ID must be set in QE out change."))?;
    sqlx::query("INSERT INTO qeout (
                        event_id,
                        change,
                        user_id,
                        source
                ) VALUES (?, ?, ?, ?)")
        .bind(event_id)
        .bind(serde_json::to_string(&rec.change).expect("convertible to JSON"))
        .bind(&rec.user_id)
        .bind(&rec.source)
        .execute(&db.0).await.map_err(sqlx_to_anyhow)?;
    
    let chng = &rec.change;
    let json_chng = serde_json::to_value(chng)
        .map_err(|e| anyhow!("serde error: {e}"))?;
    let json_chng = json_chng.as_object().expect("must be object here");
    
    info!("Applying change to runs: {json_chng:?}");
    
    // proc-macro would be fine here
    // https://www.freecodecamp.org/news/procedural-macros-in-rust/
    fn create_query_bind_json<'a>(query: &'a str, json: &'a Map<String, Value>, gen_query: &'a mut String) -> anyhow::Result<Query<'a, Sqlite, <Sqlite as Database>::Arguments<'a>>> {
        let keys = json.keys().map(|k| k.as_str()).collect::<Vec<_>>();
        let values_set = keys.iter().map(|&k| format!("{k}=?")).collect::<Vec<_>>().join(", ");
        *gen_query = query.replace("{}", &values_set);
        let q = sqlx::query(gen_query);
        fn bind_recursive<'a, 'b>(json_chng: &'a Map<String, Value>, q: Query<'a, Sqlite, <Sqlite as Database>::Arguments<'a>>, keys: &'b [&'b str]) -> Query<'a, Sqlite, <Sqlite as Database>::Arguments<'a>> {
            if let Some(&key) = keys.first() {
                let val = json_chng.get(key).expect("Value must exit here for key: {key}");
                let q = q.bind(val);
                bind_recursive(json_chng, q, &keys[1..])
            } else {
                q
            }
        }
        Ok(bind_recursive(json, q, &keys[..]))
    }
    let mut gen_query = "".to_string();
    let q = create_query_bind_json("UPDATE runs SET {} WHERE run_id=? AND event_id=?", json_chng, &mut gen_query)?;
    q.bind(run_id)
        .bind(event_id)
        .execute(&db.0).await.map_err(sqlx_to_anyhow)?;

    Ok(())
}
