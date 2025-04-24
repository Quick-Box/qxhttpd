use std::fmt::{Display, Formatter};
use anyhow::anyhow;
use chrono::{NaiveDateTime, NaiveTime, TimeDelta};
use itertools::Itertools;
use rocket::{Build, Rocket, State};
use rocket::response::status::Custom;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use rocket_dyn_templates::{context, Template};
use sqlx::{query_as, FromRow, QueryBuilder, SqlitePool};
use crate::event::{load_event_info, load_event_info_for_api_token, user_info, EventId, RunId};
use crate::{impl_sqlx_json_text_type_encode_decode, impl_sqlx_text_type_encode_decode, QxApiToken, QxSessionId, SharedQxState};
use crate::qxdatetime::QxDateTime;
use sqlx::{Encode, Sqlite};
use sqlx::query::{Query};
use sqlx::sqlite::{SqliteArgumentValue, SqliteArguments};
use log::info;
use crate::db::{get_event_db, DbPool};
use crate::oc::OCheckListChange;
use crate::util::{anyhow_to_custom_error, sqlx_to_anyhow, sqlx_to_custom_error};

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
    pub class_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_id: Option<i64>,
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
    // pub fn new(run_id: RunId) -> Self {
    //     Self {
    //         run_id,
    //         drop_record: false,
    //         class_name: None,
    //         registration: None,
    //         first_name: None,
    //         last_name: None,
    //         si_id: None,
    //         start_time: None,
    //         check_time: None,
    //         finish_time: None,
    //     }
    // }
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
        macro_rules! changed_fields {
            ($($fld_name:ident), +) => {{
                let mut ret = vec![];
                $(
                    if self.$fld_name.is_some() { ret.push(stringify!($fld_name)); }
                )*
                ret
            }}
        }
        changed_fields!(class_name, registration, first_name, last_name, si_id, start_time, check_time, finish_time)
    }

    // pub(crate) fn intersect_with_run_record(&self, rec: &RunsRecord) -> Self {
    //     macro_rules! intersect {
    //         (($($fld_name:ident), +), ($($opt_fld_name:ident), +)) => {{
    //             let mut ret = Self::new(rec.run_id);
    //             $(
    //                 if self.$fld_name.is_some() { ret.$fld_name = Some(rec.$fld_name.clone()); }
    //             )*
    //             $(
    //                 if self.$opt_fld_name.is_some() { ret.$opt_fld_name = rec.$opt_fld_name.clone(); }
    //             )*
    //             ret
    //         }}
    //     }
    //     intersect!((class_name, registration, first_name, last_name, si_id), (start_time, check_time, finish_time))
    // }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub enum ChangeStatus {
    #[default]
    Pending,
    Accepted,
    Rejected,
}
impl_sqlx_text_type_encode_decode!(ChangeStatus);

impl ChangeStatus {
    pub fn from_str(s: &str) -> Self {
        
        match s {
            stringify!(Pending) => Self::Pending,
            stringify!(Accepted) => Self::Accepted,
            stringify!(Rejected) => Self::Rejected,
            _ => panic!("Unknown status: {}", s),
        }
    }
}

impl Display for ChangeStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeStatus::Pending => f.write_str("Pending"),
            ChangeStatus::Accepted => f.write_str("Accepted"),
            ChangeStatus::Rejected => f.write_str("Rejected"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DataType {
    OcChange,
    RunUpdateRequest,
    RunUpdated,
    RadioPunch,
    CardReadout,
}

impl DataType {
    pub fn from_str(s: &str) -> Self {
        match s {
            stringify!(OcChange) => Self::OcChange,
            stringify!(RunUpdateRequest) => Self::RunUpdateRequest,
            stringify!(RunUpdated) => Self::RunUpdated,
            stringify!(RadioPunch) => Self::RadioPunch,
            stringify!(CardReadout) => Self::CardReadout,
            _ => panic!("Unknown data type: {}", s),
        }
    }
}

impl_sqlx_text_type_encode_decode!(DataType);

impl Display for DataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::OcChange => write!(f, "OcChange"),
            DataType::RunUpdateRequest => write!(f, "RunUpdateRequest"),
            DataType::RunUpdated => write!(f, "RunUpdated"),
            DataType::RadioPunch => write!(f, "RadioPunch"),
            DataType::CardReadout => write!(f, "CardReadout"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChangeData {
    Null,
    OcChange(OCheckListChange),
    RunUpdateRequest(QxRunChange),
    RunUpdated(QxRunChange),
    RadioPunch,
    CardReadout,
}
impl_sqlx_json_text_type_encode_decode!(ChangeData);
#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct ChangesRecord {
    pub id: i64,
    pub source: String,
    pub data_type: DataType,
    pub data: ChangeData,
    pub user_id: Option<String>,
    pub run_id: Option<i64>,
    pub status: Option<ChangeStatus>,
    pub created: QxDateTime,
}

pub async fn add_change(event_id: EventId, source: &str, data_type: DataType, data: &ChangeData, run_id: Option<RunId>, user_id: Option<&str>, status: Option<ChangeStatus>, state: &State<SharedQxState>) -> anyhow::Result<i64> {
    //let change = serde_json::to_value(change).map_err(|e| anyhow!("{e}"))?;
    let edb = get_event_db(event_id, state).await?;
    let id: (i64, ) = query_as("INSERT INTO changes
                (source, data_type, data, run_id, user_id, status, created)
                VALUES (?, ?, ?, ?, ?, ?, ?)  RETURNING id")
        .bind(source)
        .bind(data_type)
        .bind(data)
        .bind(run_id)
        .bind(user_id)
        .bind(status)
        .bind(QxDateTime::now().trimmed_to_sec())
        .fetch_one(&edb)
        .await.map_err(sqlx_to_anyhow)?;
    let change: ChangesRecord = query_as("SELECT * FROM changes WHERE id=?")
        .bind(id.0)
        .fetch_one(&edb)
        .await.map_err(sqlx_to_anyhow)?;
    state.read().await.broadcast_change((event_id, change)).await?;
    Ok(id.0)
}

#[get("/event/<event_id>/changes?<from_id>")]
async fn get_changes(event_id: EventId, from_id: Option<i64>, state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, gdb).await?;
    let from_id = from_id.unwrap_or(0);
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let records: Vec<ChangesRecord> = sqlx::query_as("SELECT * FROM changes WHERE id>=? ORDER BY created DESC LIMIT 1000")
        .bind(from_id)
        .fetch_all(&edb)
        .await
        .map_err(sqlx_to_custom_error)?;
    Ok(Template::render("changes", context! {
            event,
            records,
        }))
}

#[post("/api/event/<event_id>/changes/run-update-request", data = "<change>")]
pub async fn add_run_update_request_change(event_id: EventId, session_id: QxSessionId, change: Json<QxRunChange>, state: &State<SharedQxState>) -> Result<(), Custom<String>> {
    let user = user_info(session_id, state).await?;
    let change = change.into_inner();
    let data_type = DataType::RunUpdateRequest;
    let data = ChangeData::RunUpdateRequest(change.clone());
    add_change(event_id, "www", data_type, &data, Some(change.run_id), Some(user.email.as_str()), Some(ChangeStatus::Pending), state).await.map_err(anyhow_to_custom_error)?;
    state.read().await.broadcast_runs_change((event_id, change)).await.map_err(anyhow_to_custom_error)?;
    Ok(())
}

#[post("/api/event/current/changes/run-updated", data = "<change>")]
async fn add_run_updated_change(change: Json<QxRunChange>, api_token: QxApiToken, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let event = load_event_info_for_api_token(&api_token, db).await?;
    let run_change = change.into_inner();
    let run_id = run_change.run_id;
    let data_type = DataType::RunUpdated;
    let data = ChangeData::RunUpdated(run_change.clone());
    add_change(event.id, "qe", data_type, &data, Some(run_id), None, None, state).await.map_err(anyhow_to_custom_error)?;
    let db = get_event_db(event.id, state).await.map_err(anyhow_to_custom_error)?;
    apply_qe_run_change(&run_change, &db).await.map_err(anyhow_to_custom_error)?;
    Ok(())
}

async fn apply_qe_run_change(change: &QxRunChange, edb: &SqlitePool) -> anyhow::Result<()> {
    let run_id = change.run_id;
    if change.drop_record {
        sqlx::query("DELETE FROM runs WHERE run_id=?")
            .bind(run_id)
            .execute(edb).await.map_err(sqlx_to_anyhow)?;
        return Ok(())
    }
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM runs WHERE run_id=?")
        .bind(run_id)
        .fetch_one(edb).await.map_err(sqlx_to_anyhow)?;
    if count.0 == 0 {
        sqlx::query("INSERT INTO runs (
                 run_id,
                 si_id,
                 last_name,
                 first_name,
                 registration,
                 class_name,
                 start_time,
                 check_time,
                 finish_time
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(change.run_id)
            .bind(change.si_id)
            .bind(change.last_name.as_ref())
            .bind(change.first_name.as_ref())
            .bind(change.registration.as_ref())
            .bind(change.class_name.as_ref())
            .bind(change.start_time)
            .bind(change.check_time)
            .bind(change.finish_time)
            .execute(edb).await.map_err(sqlx_to_anyhow)?;
    } else {
        let changed_fields = change.changed_fields();
        let placeholders = changed_fields.iter().map(|&fld_name| format!("{fld_name}=?") ).join(",");
        let qs = format!("UPDATE runs SET {placeholders} WHERE run_id=?");
        let mut q = sqlx::query(&qs);
        fn bind_field<'a>(q: Query<'a, Sqlite, SqliteArguments<'a>>, field_name: &'a str, change: &'a QxRunChange) -> anyhow::Result<Query<'a, Sqlite, SqliteArguments<'a>>> {
            let q = if field_name == "si_id" { q.bind(change.si_id) }
            else if field_name == "first_name" { q.bind(change.first_name.as_ref()) }
            else if field_name == "last_name" { q.bind(change.last_name.as_ref()) }
            else  if field_name == "registration" { q.bind(change.registration.as_ref()) }
            else if field_name == "class_name" { q.bind(change.class_name.as_ref()) }
            else if field_name == "start_time" { q.bind(change.start_time) }
            else if field_name == "check_time" { q.bind(change.check_time) }
            else if field_name == "finish_time" { q.bind(change.finish_time) }
            else {
                return Err(anyhow!("Dont know how to bind field {field_name}"))
            };
            Ok(q)
        }
        for field_name in changed_fields {
            q = bind_field(q, field_name, change)?;
        }
        let q = q.bind(run_id);
        q.execute(edb).await.map_err(sqlx_to_anyhow)?;
    }
    Ok(())
}

#[get("/api/event/<event_id>/changes/sse")]
async fn changes_sse(event_id: EventId, state: &State<SharedQxState>) -> EventStream![] {
    let mut chng_receiver = state.read().await.changes_receiver.clone();
    EventStream! {
        loop {
            let (chng_event_id, change) = match chng_receiver.recv().await {
                Ok(chng) => chng,
                Err(e) => {
                    error!("Read change record error: {e}");
                    break;
                }
            };
            if event_id == chng_event_id {
                match serde_json::to_string(&change) {
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

#[get("/api/event/<event_id>/changes?<from_id>&<data_type>&<status>")]
async fn api_get_changes(event_id: EventId, from_id: Option<i64>, data_type: Option<&str>, status: Option<&str>, state: &State<SharedQxState>) -> Result<Json<Vec<ChangesRecord>>, Custom<String>> {
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;

    let mut query_builder = QueryBuilder::new("SELECT * FROM changes WHERE id>=");
    query_builder.push_bind(from_id.unwrap_or(0));
    if let Some(data_type) = data_type {
        query_builder.push(" AND data_type=");
        query_builder.push_bind(data_type);
    }
    if let Some(status) = status {
        query_builder.push(" AND status=");
        query_builder.push_bind(status);
    }
    query_builder.push(" ORDER BY created");
    
    let query = query_builder.build_query_as::<ChangesRecord>();
    let records: Vec<_> = query.fetch_all(&edb).await.map_err(sqlx_to_custom_error)?;
    info!("records: {:?}", records);
    Ok(records.into())
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
        get_changes,
        changes_sse,
        add_run_updated_change,
        add_run_update_request_change,
        api_get_changes,
    ])
}