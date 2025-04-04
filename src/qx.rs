use chrono::{NaiveDateTime, NaiveTime, TimeDelta};
use rocket::response::status::Custom;
use rocket::{Build, Rocket, State};
use rocket::serde::{Deserialize, Serialize};
use rocket_dyn_templates::{context, Template};
use crate::event::{load_event_info, EventId};
use crate::oc::OCheckListChange;
use crate::qxdatetime::QxDateTime;
use crate::changes::ChangesRecord;
use crate::db::{get_event_db, DbPool};
use crate::SharedQxState;
use crate::util::{anyhow_to_custom_error, sqlx_to_custom_error};

fn is_false(b: &bool) -> bool {
    *b == false
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct QxRunChange {
    pub run_id: i64,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub drop_record: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_id: Option<i64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_time: Option<QxDateTime>,
}
impl QxRunChange {
    pub fn try_from_oc_change(oc: &OCheckListChange, change_set_created_time: QxDateTime) -> anyhow::Result<Self> {
        let mut change = Self {
            run_id: oc.Runner.Id.parse::<i64>()?,
            ..Default::default()
        };
        if let Some(start_time) = &oc.Runner.StartTime {
            // start time can be 10:20:30 or 25-05-01T10:20:03+01:00 depending on version of OCheckList
            change.check_time = if start_time.len() == 8 {
                let tm = NaiveTime::parse_from_str(start_time, "%H:%M:%S")?;
                let dt = change_set_created_time.0.date_naive();
                let dt = NaiveDateTime::new(dt, tm);
                QxDateTime::from_local_timezone(dt, change_set_created_time.0.offset())
            } else {
                QxDateTime::parse_from_string(start_time, Some(&change_set_created_time.0.offset()))?.0
                    // estimate check time to be 2 minutes before start time
                    .checked_sub_signed(TimeDelta::minutes(2))
                    .map(|dt| QxDateTime(dt))
            };
        }
        change.si_id = oc.Runner.NewCard;
        if let Some(change_log) = &oc.ChangeLog {
            if let Some(dtstr) = change_log.get("Late start") {
                // take check time from change log
                let dt = QxDateTime::parse_from_string(dtstr, None)?;
                change. check_time = Some(dt);
            }
            if let Some(_dtstr) = change_log.get("DNS") {
                // no start - no check
                change.check_time = None;
            }
        }
        Ok(change)
    }
    pub fn changed_fields(&self) -> Vec<&str> {
        let mut ret = vec![];
        if self.first_name.is_some() { ret.push("first_name") }
        if self.last_name.is_some() { ret.push("last_name") }
        if self.class_name.is_some() { ret.push("class_name") }
        if self.si_id.is_some() { ret.push("si_id") }
        if self.registration.is_some() { ret.push("registration") }
        if self.start_time.is_some() { ret.push("start_time") }
        if self.check_time.is_some() { ret.push("check_time") }
        if self.finish_time.is_some() { ret.push("finish_time") }
        ret
    }
}
// pub async fn import_runs(event_id: EventId, db: &State<DbPool>) -> anyhow::Result<()> {
//     let data = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND name=?")
//         .bind(event_id)
//         .bind(RUNS_CSV_FILE)
//         .fetch_one(&db.0)
//         .await.map_err(sqlx_to_anyhow)?.0;
//     let runs: Vec<RunsRecord> = serde_json::from_slice(&data)?;
// 
//     let mut run_ids = sqlx::query_as::<_, (i64,)>("SELECT run_id FROM runs WHERE event_id=?")
//         .bind(event_id)
//         .fetch_all(&db.0)
//         .await.map_err(sqlx_to_anyhow)?;
// 
//     let txn = db.0.begin().await?;
// 
//     for run in runs {
//         run_ids.retain(|n| n.0 != run.run_id);
//         sqlx::query("INSERT OR REPLACE INTO runs (
//                              event_id,
//                              run_id,
//                              si_id,
//                              last_name,
//                              first_name,
//                              registration,
//                              class_name,
//                              start_time,
//                              check_time,
//                              finish_time,
//                              status
//                              ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
//             .bind(event_id)
//             .bind(run.run_id)
//             .bind(run.si_id)
//             .bind(run.last_name)
//             .bind(run.first_name)
//             .bind(run.registration)
//             .bind(run.class_name)
//             .bind(run.start_time.map(|d| d.0))
//             .bind(run.check_time.map(|d| d.0))
//             .bind(run.finish_time.map(|d| d.0))
//             .bind(run.status)
//             .execute(&db.0).await.map_err(sqlx_to_anyhow)?;
//     }
//     for run_id in run_ids {
//         sqlx::query("DELETE FROM runs WHERE run_id=? AND event_id=?")
//             .bind(run_id.0)
//             .bind(event_id)
//             .execute(&db.0).await.map_err(sqlx_to_anyhow)?;
//     }
// 
//     txn.commit().await?;
// 
//     Ok(())
// }
#[get("/event/<event_id>/changes?<from_id>")]
async fn get_changes(event_id: EventId, from_id: Option<i64>, state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, gdb).await?;
    let from_id = from_id.unwrap_or(0);
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let records: Vec<ChangesRecord> = sqlx::query_as("SELECT * FROM changes WHERE id>=? LIMIT 1000")
        .bind(from_id)
        .fetch_all(&edb)
        .await
        .map_err(sqlx_to_custom_error)?;
    Ok(Template::render("changes", context! {
            event,
            records,
        }))
}


pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
        get_changes,
    ])
}


