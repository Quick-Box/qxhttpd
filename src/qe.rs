use std::collections::BTreeMap;
use std::str::FromStr;
use chrono::{DateTime, FixedOffset, NaiveDateTime, NaiveTime, TimeZone};
use rocket::http::Status;
use rocket::response::status;
use rocket::response::status::Custom;
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use rocket::{Build, Rocket, State};
use rocket::response::stream::{Event, EventStream};
use rocket_dyn_templates::{context, Template};
use sqlx::{query};
use crate::db::DbPool;
use crate::event::{load_event_info, load_event_info2, user_info, EventId, RunId, SiId, START_LIST_IOFXML3_FILE};
use crate::{impl_sqlx_json_text_type_and_decode, iofxml3, QxApiToken, QxSessionId, SharedQxState};
use crate::iofxml3::structs::StartList;
use crate::oc::OCheckListChange;
use crate::qe::classes::ClassesRecord;
use crate::qe::runs::{apply_qe_out_change, RunsRecord};
use crate::qxdatetime::QxDateTime;
use crate::util::{anyhow_to_custom_error, sqlx_to_anyhow, sqlx_to_custom_error};

pub mod runs;
pub mod classes;

fn start_list_start00(stlist: &StartList) -> Option<NaiveDateTime> {
    let d = stlist.event.start_time.date.as_str();
    let t = stlist.event.start_time.time.as_str();
    NaiveDateTime::from_str(&format!("{d}T{t}")).ok()
}
pub async fn import_startlist(event_id: EventId, db: &State<DbPool>) -> anyhow::Result<()> {
    let data = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND name=?")
        .bind(event_id)
        .bind(START_LIST_IOFXML3_FILE)
        .fetch_one(&db.0)
        .await.map_err(sqlx_to_anyhow)?.0;
    
    let (start00, classes, runs) = parse_startlist_xml_data(event_id, data).await?;

    let txn = db.0.begin().await?;
    sqlx::query("UPDATE events SET start_time=? WHERE id=?")
        .bind(start00)
        .bind(event_id)
        .execute(&db.0).await.map_err(sqlx_to_anyhow)?;
    for cr in classes {
        sqlx::query("INSERT OR REPLACE INTO classes (event_id, name, length, climb, control_count) VALUES (?, ?, ?, ?, ?)")
            .bind(event_id)
            .bind(cr.name)
            .bind(cr.length)
            .bind(cr.climb)
            .bind(cr.control_count)
            .execute(&db.0).await.map_err(sqlx_to_anyhow)?;
    }
    for run in runs {
        sqlx::query("INSERT OR REPLACE INTO runs (
                             event_id,
                             run_id,
                             si_id,
                             last_name,
                             first_name,
                             registration,
                             class_name,
                             start_time,
                             check_time,
                             finish_time,
                             status
                             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(event_id)
            .bind(run.run_id)
            .bind(run.si_id)
            .bind(run.last_name)
            .bind(run.first_name)
            .bind(run.registration)
            .bind(run.class_name)
            .bind(run.start_time.map(|d| d.0))
            .bind(run.check_time.map(|d| d.0))
            .bind(run.finish_time.map(|d| d.0))
            .bind(run.status)
            .execute(&db.0).await.map_err(sqlx_to_anyhow)?;
    }
    txn.commit().await?;

    Ok(())
}
pub async fn parse_startlist_xml_data(event_id: EventId, data: Vec<u8>) -> anyhow::Result<(Option<DateTime<FixedOffset>>, Vec<ClassesRecord>, Vec<RunsRecord>)> {
    let stlist = iofxml3::parser::parse_startlist(&data)?;
    let start00_naive = start_list_start00(&stlist).ok_or(anyhow::anyhow!("Invalid start list date time"))?;
    let mut fixed_offset = None;
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
                start_time: Default::default(),
                interval: 0,
                start_slot_count: 0,
            };
            classes.insert(class_name.clone(), classrec);
        }
        for ps in &cs.person_start {
            let mut runsrec = RunsRecord { class_name: class_name.clone(), ..Default::default() };
            let person = &ps.person;
            let name = &person.name;
            runsrec.first_name = name.given.to_string();
            runsrec.last_name = name.family.to_string();
            runsrec.registration = person.id.iter().find(|id| id.id_type == "CZE")
                .and_then(|id| id.text.clone()).unwrap_or_default();
            let Some(run_id) = person.id.iter().find(|id| id.id_type == "QuickEvent") else {
                warn!("QuickEvent ID not found in person_start {:?}", ps);
                continue;
            };
            let Some(run_id) = run_id.text.as_ref().and_then(|id| id.parse::<i64>().ok()) else {
                // still can be a vacant
                if !runsrec.registration.is_empty() {
                    warn!("QuickEvent ID value invalid: {:?}", ps);
                }
                continue;
            };
            runsrec.run_id = run_id;
            let Ok(start_time) = QxDateTime::from_iso_string(&ps.start.start_time) else {
                warn!("Start time value invalid: {:?}", ps);
                continue;
            };
            if fixed_offset.is_none() {
                fixed_offset = Some(*start_time.0.offset());
            }
            runsrec.start_time = Some(start_time);
            let si = &ps.start.control_card.as_ref().and_then(|si| si.parse::<i64>().ok()).unwrap_or_default();
            runsrec.si_id = *si;
            runs.push(runsrec);
        }
    }
    let classes = classes.into_values().collect();
    let start00 = fixed_offset.and_then(|offset| offset.from_local_datetime(&start00_naive).single());
    Ok((start00, classes, runs))
}

#[derive(Serialize, Deserialize, sqlx::FromRow, Clone, Debug)]
pub struct QEJournalRecord {
    pub id: i64,
    pub event_id: EventId,
    //#[serde(default)]
    //pub original: Option<QERunRecord>,
    pub change: QERunChange,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub user_id: Option<String>,
    created: DateTime<chrono::Utc>,
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
    pub class_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}
impl QERunChange {
    pub fn try_from_oc_change(start00: &QxDateTime, oc: &OCheckListChange) -> Result<Self, String> {
        let tm = NaiveTime::parse_from_str(&oc.Runner.StartTime, "%H:%M:%S")
            .map_err(|e| warn!("Invalid start time {}, parse error: {e}", oc.Runner.StartTime)).ok();
        let dt = tm.and_then(|tm| QxDateTime::from_local_timezone(NaiveDateTime::new(start00.0.date_naive(), tm), start00.0.offset()));
        Ok(QERunChange {
            run_id: oc.Runner.Id.parse::<i64>().ok(),
            si_id: if oc.Runner.Card > 0 {Some(oc.Runner.Card)} else {None},
            class_name: None,
            first_name: None,
            last_name: None,
            registration: None,
            check_time: dt,
            start_time: None,
            finish_time: None,
            status: None,
            //comment: if oc.Runner.Comment.is_empty() { None } else { Some(oc.Runner.Comment.clone()) },
        })
    }
}
impl_sqlx_json_text_type_and_decode!(QERunChange);

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
#[post("/api/qe/<event_id>/out/changes", data = "<change_set>")]
async fn post_api_qe_out(event_id: EventId, change_set: Json<QERunChange>, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    add_qe_out_change_record(event_id, "qe", Some(&user.email), &change_set, db).await;
    Ok(())
}
#[get("/event/<event_id>/qe/in/changes")]
async fn get_event_qe_in_chng(event_id: EventId, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let pool = &db.0;
    let records: Vec<QEJournalRecord> = sqlx::query_as("SELECT * FROM qein WHERE event_id=?")
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Template::render("qe-in", context! {
            event,
            records,
        }))
}
#[get("/api/event/<event_id>/qe/in/changes/sse")]
fn get_api_event_qe_in_chng_sse(event_id: EventId, state: &State<SharedQxState>) -> EventStream![] {
    let mut chng_receiver = state.read().unwrap().qe_in_changes_sender.subscribe();
    EventStream! {
        loop {
            let in_rec = match chng_receiver.recv().await {
                Ok(chng) => chng,
                Err(e) => {
                    error!("Receive QE in record error: {e}");
                    break;
                }
            };
            if event_id == in_rec.event_id {
                match serde_json::to_string(&in_rec) {
                    Ok(json) => {
                        yield Event::data(json);
                    }
                    Err(e) => {
                        error!("Serde error: {e}");
                        break;
                    }
                }
            }
        }
    }
}
#[post("/api/event/<event_id>/qe/in/changes", data = "<change>")]
fn request_run_change(event_id: EventId, change: Json<QERunChange>, session_id: QxSessionId, state: &State<SharedQxState>) -> Result<(), Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    let rec = QEJournalRecord {
        id: 0,
        event_id,
        change: change.into_inner(),
        source: "browser".into(),
        user_id: Some(user.email),
        created: chrono::Utc::now(),
    };
    if let Err(e) = state.read().expect("not poisoned")
        .broadcast_qe_in_run_change(rec) {
        error!("Failed to send QE in record error: {e}");
    }
    Ok(())
}

#[post("/api/event/current/qe/out/changes", data = "<change>")]
async fn qe_change_applied(change: Json<QERunChange>, api_token: QxApiToken, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let event = load_event_info2(&api_token, db).await?;
    let rec = QEJournalRecord {
        id: 0,
        event_id: event.id,
        change: change.into_inner(),
        source: "qe".into(),
        user_id: None,
        created: chrono::Utc::now(),
    };
    apply_qe_out_change(&rec, db).await.map_err(anyhow_to_custom_error)?;
    if let Err(e) = state.read().expect("not poisoned")
        .broadcast_qe_out_run_change(rec) {
        error!("Failed to send QE in record error: {e}");
    }
    Ok(())
}
#[get("/api/event/<event_id>/qe/out/changes/sse")]
fn get_api_event_qe_out_chng_sse(event_id: EventId, state: &State<SharedQxState>) -> EventStream![] {
    let mut chng_receiver = state.read().unwrap().qe_in_changes_sender.subscribe();
    EventStream! {
        loop {
            let in_rec = match chng_receiver.recv().await {
                Ok(chng) => chng,
                Err(e) => {
                    error!("Receive QE in record error: {e}");
                    break;
                }
            };
            if event_id == in_rec.event_id {
                match serde_json::to_string(&in_rec) {
                    Ok(json) => {
                        yield Event::data(json);
                    }
                    Err(e) => {
                        error!("Serde error: {e}");
                        break;
                    }
                }
            }
        }
    }
}

#[get("/api/event/<event_id>/runs?<run_id>&<class_name>")]
async fn get_event_runs(event_id: EventId, class_name: Option<&str>, run_id: Option<i32>, db: &State<DbPool>) -> Result<Json<Vec<RunsRecord>>, Custom<String>> {
    //let event = load_event_info(event_id, db).await?;
    let run_id_filter = run_id.map(|id| format!("AND run_id={id}")).unwrap_or_default();
    let class_filter = class_name.map(|n| format!("AND class_name='{n}'")).unwrap_or_default();
    let qs = format!("SELECT * FROM runs WHERE event_id=? {run_id_filter} {class_filter} ORDER BY run_id");
    let runs = sqlx::query_as::<_, RunsRecord>(&qs)
        .bind(event_id)
        .fetch_all(&db.0).await.map_err(sqlx_to_custom_error)?;
    Ok(runs.into())
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
        get_event_qe_in_chng,
        get_api_event_qe_in_chng_sse,
        request_run_change,
        post_api_qe_out,
        qe_change_applied,
        get_api_event_qe_out_chng_sse,
        get_event_runs,
    ])
}