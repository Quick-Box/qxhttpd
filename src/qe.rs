use std::collections::BTreeMap;
use chrono::NaiveDateTime;
use rocket::http::Status;
use rocket::response::status;
use rocket::response::status::Custom;
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use rocket::{Build, Rocket, State};
use rocket_dyn_templates::{context, Template};
use sqlx::query;
use crate::db::DbPool;
use crate::event::{load_event_info, load_event_info2, EventId, RunId, SiId, START_LIST_IOFXML3_FILE};
use crate::{impl_sqlx_json_text_type_and_decode, iofxml3, QxApiToken};
use crate::iofxml3::structs::StartList;
use crate::oc::OCheckListChange;
use crate::qe::classes::ClassesRecord;
use crate::qe::runs::RunsRecord;
use crate::util::{try_parse_naive_datetime, tee_sqlx_error};

pub mod runs;
pub mod classes;

fn start00(stlist: &StartList) -> Option<NaiveDateTime> {
    let d = stlist.event.start_time.date.as_str();
    let t = stlist.event.start_time.time.as_str();
    try_parse_naive_datetime(&format!("{d}T{t}"))
}
pub async fn import_startlist(event_id: EventId, db: &State<DbPool>) -> anyhow::Result<()> {
    let data = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND name=?")
        .bind(event_id)
        .bind(START_LIST_IOFXML3_FILE)
        .fetch_one(&db.0)
        .await.map_err(tee_sqlx_error)?.0;
    
    let (start00, classes, runs) = parse_startlist_xml_data(event_id, data).await?;

    let txn = db.0.begin().await?;
    sqlx::query("UPDATE events SET start_time=? WHERE id=?")
        .bind(start00)
        .bind(event_id)
        .execute(&db.0).await.map_err(tee_sqlx_error)?;
    for cr in classes {
        sqlx::query("INSERT OR REPLACE INTO classes (event_id, name, length, climb, control_count) VALUES (?, ?, ?, ?, ?)")
            .bind(event_id)
            .bind(cr.name)
            .bind(cr.length)
            .bind(cr.climb)
            .bind(cr.control_count)
            .execute(&db.0).await.map_err(tee_sqlx_error)?;
    }
    for run in runs {
        sqlx::query("INSERT OR REPLACE INTO runs (
                             event_id,
                             run_id,
                             si_id,
                             runner_name,
                             registration,
                             class_name,
                             start_time,
                             check_time,
                             finish_time,
                             status
                             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(event_id)
            .bind(run.run_id)
            .bind(run.si_id)
            .bind(run.runner_name)
            .bind(run.registration)
            .bind(run.class_name)
            .bind(run.start_time)
            .bind(run.check_time)
            .bind(run.finish_time)
            .bind(run.status)
            .execute(&db.0).await.map_err(tee_sqlx_error)?;
    }
    txn.commit().await?;

    Ok(())
}
pub async fn parse_startlist_xml_data(event_id: EventId, data: Vec<u8>) -> anyhow::Result<(NaiveDateTime, Vec<ClassesRecord>, Vec<RunsRecord>)> {
    let stlist = iofxml3::parser::parse_startlist(&data)?;
    let start00 = start00(&stlist).ok_or(anyhow::anyhow!("Invalid start list date time"))?;
    let mut runs = Vec::new();
    let mut classes = BTreeMap::new();
    for cs in &stlist.class_start {
        let class_name = cs.class.name.clone();
        if !classes.contains_key(&class_name) {
            let classrec = ClassesRecord {
                id: 0,
                event_id,
                name: class_name.clone(),
                length: cs.course.length.parse::<i64>().unwrap_or(0),
                climb: cs.course.climb.parse::<i64>().unwrap_or(0),
                control_count: cs.course.number_of_controls.parse::<i64>().unwrap_or(0),
            };
            classes.insert(class_name.clone(), classrec);
        }
        for ps in &cs.person_start {
            let mut runsrec = RunsRecord::default();
            runsrec.class_name = class_name.clone();
            let person = &ps.person;
            let name = &person.name;
            runsrec.runner_name = format!("{} {}", name.family, name.given);
            runsrec.registration = person.id.iter().find(|id| id.id_type == "CZE")
                .map(|id| id.text.clone()).flatten().unwrap_or_default();
            let Some(run_id) = person.id.iter().find(|id| id.id_type == "QuickEvent") else {
                warn!("QuickEvent ID not found in person_start {:?}", ps);
                continue;
            };
            let Some(run_id) = run_id.text.as_ref().map(|id| id.parse::<i64>().ok()).flatten() else {
                warn!("QuickEvent ID value invalid: {:?}", ps);
                continue;
            };
            runsrec.run_id = run_id;
            let Some(start_time) = try_parse_naive_datetime(&ps.start.start_time) else {
                warn!("Start time value invalid: {:?}", ps);
                continue;
            };
            runsrec.start_time = start_time;
            let si = &ps.start.control_card.as_ref().map(|si| si.parse::<i64>().ok()).flatten().unwrap_or_default();
            runsrec.si_id = *si;
            runs.push(runsrec);
        }
    }
    let classes = classes.into_iter().map(|(_, v)| v).collect();
    Ok((start00, classes, runs))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct QERunRecord {
    pub id: RunId,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub runner_name: String,
    #[serde(default)]
    pub si_id: SiId,
    #[serde(default)]
    pub check_time: String,
    #[serde(default)]
    pub start_time: i64,
    #[serde(default)]
    pub comment: String,
}
#[derive(Serialize, Deserialize, sqlx::FromRow, Clone, Debug)]
pub struct QEInRecord {
    pub id: i64,
    pub event_id: EventId,
    //#[serde(default)]
    //pub original: Option<QERunRecord>,
    pub change: QERunChange,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub user_id: String,
    created: chrono::DateTime<chrono::Utc>,
}
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct QERunChange {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_id: Option<SiId>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_time: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<i64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl_sqlx_json_text_type_and_decode!(QERunChange);

impl TryFrom<&OCheckListChange> for QERunChange {
    type Error = String;

    fn try_from(oc: &OCheckListChange) -> Result<Self, Self::Error> {
        Ok(QERunChange {
            run_id: oc.Runner.Id.parse::<i64>().ok(),
            si_id: if oc.Runner.Card > 0 {Some(oc.Runner.Card)} else {None},
            check_time: if oc.Runner.StartTime.is_empty() {None} else {Some(oc.Runner.StartTime.clone())},
            start_time: None,
            comment: if oc.Runner.Comment.is_empty() { None } else { Some(oc.Runner.Comment.clone()) },
        })
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct QERadioRecord {
    pub siId: SiId,
    #[serde(default)]
    pub time: String,
}

pub async fn add_qe_out_change_record(event_id: EventId, source: &str, user_id: Option<&str>, change: &QERunChange, db: &State<DbPool>) {
    let Ok(change) = serde_json::to_string(change) else {
        error!("Serde error");
        return
    };
    let _ = query("INSERT INTO qeout
    (event_id, change, source, user_id)
    VALUES (?, ?, ?, ?)")
        .bind(event_id)
        .bind(change)
        .bind(source)
        .bind(user_id)
        .execute(&db.0)
        .await.map_err(|e| warn!("Insert QE in record error: {e}"));
}
pub async fn add_qe_in_change_record(event_id: EventId, source: &str, user_id: Option<&str>, change: &QERunChange, db: &State<DbPool>) {
    let Ok(change) = serde_json::to_string(change) else {
        error!("Serde error");
        return
    };
    let _ = query("INSERT INTO qein
    (event_id, change, source, user_id)
    VALUES (?, ?, ?, ?)")
        .bind(event_id)
        .bind(change)
        .bind(source)
        .bind(user_id)
        .execute(&db.0)
        .await.map_err(|e| warn!("Insert QE in record error: {e}"));
}
#[post("/api/token/qe/out", data = "<change_set>")]
async fn post_api_token_oc_out(api_token: QxApiToken, change_set: Json<QERunChange>, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let event = load_event_info2(&api_token, db).await?;
    add_qe_out_change_record(event.id, "qe", None, &change_set, db).await;
    Ok(())
}
#[get("/event/<event_id>/qe/in")]
async fn get_qe_in(event_id: EventId, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let pool = &db.0;
    let records: Vec<QEInRecord> = sqlx::query_as("SELECT * FROM qein WHERE event_id=?")
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Template::render("qe-in", context! {
            event,
            records,
        }))
}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_qe_in,
            post_api_token_oc_out,
        ])
}