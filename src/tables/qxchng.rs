use std::collections::{BTreeMap, HashMap};
use rocket_dyn_templates::{context, Template};
use std::fmt::{Display, Formatter};
use chrono::{FixedOffset, TimeDelta};
use rocket::response::status::{Custom};
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use rocket::{Build, Rocket, State};
use sqlx::{query, FromRow};
use crate::event::{load_event_info, load_event_info_for_api_token, user_info, EventId, RunId, RUNS_CSV_FILE, START_LIST_IOFXML3_FILE};
use crate::{impl_sqlx_json_text_type_encode_decode, iofxml3, QxApiToken, QxSessionId, SharedQxState};
use crate::db::{get_event_db, DbPool};
use crate::oc::OCheckListChange;
use crate::qe::QeChange;
use crate::qxdatetime::QxDateTime;
use crate::tables::classes::ClassesRecord;
use crate::tables::runs::{commit_qe_change, RunsRecord};
use crate::util::{anyhow_to_custom_error, sqlx_to_anyhow, sqlx_to_custom_error};
use sqlx::sqlite::SqliteArgumentValue;
use sqlx::{Encode, Sqlite};

#[derive(Serialize, Deserialize, Debug)]
pub enum QxChngStatus {
    #[serde(rename = "ACC")]
    Accepted,
    #[serde(rename = "REJ")]
    Rejected,
}
impl_sqlx_json_text_type_encode_decode!(QxChngStatus);

impl Display for QxChngStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            QxChngStatus::Accepted => f.write_str("ACC"),
            QxChngStatus::Rejected => f.write_str("REJ"),
        }
    }
}

#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct QxChngRecord {
    pub id: i64,
    pub property: String,
    pub value: String,
    pub status: Option<QxChngStatus>,
    pub user_id: String,
    pub created: QxDateTime,
}

pub async fn import_runs(event_id: EventId, db: &State<DbPool>) -> anyhow::Result<()> {
    let data = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND name=?")
        .bind(event_id)
        .bind(RUNS_CSV_FILE)
        .fetch_one(&db.0)
        .await.map_err(sqlx_to_anyhow)?.0;
    let runs: Vec<RunsRecord> = serde_json::from_slice(&data)?;

    let mut run_ids = sqlx::query_as::<_, (i64,)>("SELECT run_id FROM runs WHERE event_id=?")
        .bind(event_id)
        .fetch_all(&db.0)
        .await.map_err(sqlx_to_anyhow)?;

    let txn = db.0.begin().await?;

    for run in runs {
        run_ids.retain(|n| n.0 != run.run_id);
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
    for run_id in run_ids {
        sqlx::query("DELETE FROM runs WHERE run_id=? AND event_id=?")
            .bind(run_id.0)
            .bind(event_id)
            .execute(&db.0).await.map_err(sqlx_to_anyhow)?;
    }

    txn.commit().await?;

    Ok(())
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
pub async fn parse_startlist_xml_data(event_id: EventId, data: Vec<u8>) -> anyhow::Result<(Option<QxDateTime>, Vec<ClassesRecord>, Vec<RunsRecord>)> {
    let stlist = iofxml3::parser::parse_startlist(&data)?;
    let start_00_str = format!("{}T{}", stlist.event.start_time.date, stlist.event.start_time.time);
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
            let Ok(start_time) = QxDateTime::parse_from_iso(&ps.start.start_time) else {
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
    let start00 = QxDateTime::parse_from_string(&start_00_str, fixed_offset.as_ref()).ok();
    Ok((start00, classes, runs))
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub enum QxValChange {
    #[default]
    Null,
    Text(String),
    Number(i64),
    DateTime(QxDateTime),
}
impl_sqlx_json_text_type_encode_decode!(QxValChange);

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct QxChange {
    pub run_id: RunId,
    pub property: String,
    #[serde(default)]
    pub value: QxValChange,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_id: Option<i64>,
}
// impl_sqlx_json_text_type_encode_decode!(QxChange);

impl QxChange {
    pub fn try_from_oc_change(oc: &OCheckListChange, local_offset: Option<&FixedOffset>) -> anyhow::Result<Vec<Self>> {
        const SI_ID: &str = "si_id";
        const CHECK_TIME: &str = "check_time";
        let mut changes = HashMap::new();
        let run_id = oc.Runner.Id.parse::<i64>()?;
        let dt = QxDateTime::parse_from_string(&oc.Runner.StartTime, local_offset)?.0
            .checked_sub_signed(TimeDelta::minutes(2)); // estimate check time to be 2 minutes before start time
        if let Some(dt) = dt {
            changes.insert(CHECK_TIME.to_string(), QxValChange::DateTime(QxDateTime(dt)));
        }
        if let Some(change_log) = &oc.ChangeLog {
            if change_log.contains_key("NewCard") {
                changes.insert(SI_ID.to_string(), QxValChange::Number(oc.Runner.Card));
            }
            if let Some(dtstr) = change_log.get("Late start") {
                // take check time from change log
                let dt = QxDateTime::parse_from_string(dtstr, None)?;
                changes.insert(CHECK_TIME.to_string(), QxValChange::DateTime(dt));
            }
            if let Some(_dtstr) = change_log.get("DNS") {
                // no start - no check
                changes.remove(CHECK_TIME);
            }
        }
        Ok(changes.into_iter().map(|(property, value)| {
            QxChange {
                run_id,
                property,
                value,
                change_id: None,
            }
        }).collect())
    }
}

// #[derive(Serialize, Deserialize, Clone, Debug)]
// #[allow(non_snake_case)]
// pub struct QERadioRecord {
//     pub siId: SiId,
//     #[serde(default)]
//     pub time: String,
// }
//

pub async fn add_qx_change(event_id: EventId, user_id: Option<&str>, change: &QxChange, state: &State<SharedQxState>) -> anyhow::Result<()> {
    let run_id = change.run_id;
    //let change = serde_json::to_value(change).map_err(|e| anyhow!("{e}"))?;
    let db = get_event_db(event_id, state).await?;
    query("INSERT INTO qxchng
                (run_id, property, value, user_id, created)
                VALUES (?, ?, ?, ?, ?)")
        .bind(run_id)
        .bind(&change.property)
        .bind(&change.value)
        .bind(user_id)
        .bind(QxDateTime::now().trimmed_to_sec().0)
        .execute(&db)
        .await.map_err(sqlx_to_anyhow)?;
    Ok(())
}
#[get("/event/<event_id>/qx/changes?<from_id>")]
async fn get_qx_changes(event_id: EventId, from_id: Option<i64>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let pool = &db.0;
    let from_id = from_id.unwrap_or(0);
    let records: Vec<QxChngRecord> = sqlx::query_as("SELECT * FROM qxchng WHERE id>=?")
        .bind(event_id)
        .bind(from_id)
        .fetch_all(pool)
        .await
        .map_err(sqlx_to_custom_error)?;
    Ok(Template::render("tables-in", context! {
            event,
            records,
        }))
}

#[post("/api/event/<event_id>/runs/changes", data = "<change>")]
async fn request_run_change(event_id: EventId, change: Json<QxChange>, session_id: QxSessionId, state: &State<SharedQxState>) -> Result<(), Custom<String>> {
    let user = user_info(session_id, state)?;
    let change = change.into_inner();
    add_qx_change(event_id, Some(user.email.as_str()), &change, state).await.map_err(anyhow_to_custom_error)?;
    if let Err(e) = state.read().expect("not poisoned")
        .broadcast_runs_change((event_id, change)) {
        error!("Failed to send QE in record error: {e}");
    }
    Ok(())
}


#[post("/api/event/current/qe/changes", data = "<change>")]
async fn apply_qe_change(change: Json<QeChange>, api_token: QxApiToken, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let event = load_event_info_for_api_token(&api_token, db).await?;
    let chng = change.into_inner();
    match chng {
        QeChange::RunEdit(run_edit) => {
            if let Some((change_id, status)) = run_edit.qx_change {
                commit_qe_change(event.id, change_id, status, state).await.map_err(anyhow_to_custom_error)?;
            }
        }
        QeChange::RemotePunch(_si_id, _date_time) => {
            // TODO: RemotePunch implementation
        }
    }
    Ok(())
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
        request_run_change,
        get_qx_changes,
        apply_qe_change,
        get_event_runs,
    ])
}
