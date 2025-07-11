use std::fmt::{Display, Formatter};
use anyhow::anyhow;
use itertools::Itertools;
use rocket::{Build, Rocket, State};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use rocket_dyn_templates::{context, Template};
use sqlx::{query_as, FromRow, QueryBuilder, SqlitePool};
use crate::event::{load_event_info, load_event_info_for_api_token, user_info, user_info_opt, EventId};
use crate::{impl_sqlx_json_text_type_encode_decode, impl_sqlx_text_type_encode_decode, MaybeSessionId, QxApiToken, QxSessionId, SharedQxState};
use crate::qxdatetime::QxDateTime;
use sqlx::{Encode, Sqlite};
use sqlx::query::{Query};
use sqlx::sqlite::{SqliteArgumentValue, SqliteArguments};
use crate::db::{get_event_db, DbPool};
use crate::oc::OCheckListChange;
use crate::runs::{RunChange};
use crate::util::{anyhow_to_custom_error, sqlx_to_anyhow, sqlx_to_custom_error};

pub(crate) type DataId = i64;

pub const PENDING: &str = "Pending";
const ACCEPTED: &str = "Accepted";
const REJECTED: &str = "Rejected";
const LOCKED: &str = "Locked";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub enum ChangeStatus {
    #[default]
    Pending,
    Locked,
    Accepted,
    Rejected,
}
impl_sqlx_text_type_encode_decode!(ChangeStatus);

impl ChangeStatus {
    pub fn from_str(s: &str) -> Self {
        
        match s {
            PENDING => Self::Pending,
            ACCEPTED => Self::Accepted,
            REJECTED => Self::Rejected,
            _ => panic!("Unknown status: {}", s),
        }
    }
}

impl Display for ChangeStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeStatus::Pending => f.write_str(PENDING),
            ChangeStatus::Accepted => f.write_str(ACCEPTED),
            ChangeStatus::Rejected => f.write_str(REJECTED),
            ChangeStatus::Locked => f.write_str(LOCKED),
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

const OC_CHANGE: &str = "OcChange";
pub const RUN_UPDATE_REQUEST: &str = "RunUpdateRequest";
const RUN_UPDATED: &str = "RunUpdated";
const RADIO_PUNCH: &str = "RadioPunch";
const CARD_READOUT: &str = "CardReadout";

impl DataType {
    pub fn from_str(s: &str) -> Self {
        match s {
            OC_CHANGE => Self::OcChange,
            RUN_UPDATE_REQUEST => Self::RunUpdateRequest,
            RUN_UPDATED => Self::RunUpdated,
            RADIO_PUNCH => Self::RadioPunch,
            CARD_READOUT => Self::CardReadout,
            _ => panic!("Unknown data type: {}", s),
        }
    }
}

impl_sqlx_text_type_encode_decode!(DataType);

impl Display for DataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::OcChange => write!(f, "{}", OC_CHANGE),
            DataType::RunUpdateRequest => write!(f, "{}", RUN_UPDATE_REQUEST),
            DataType::RunUpdated => write!(f, "{}", RUN_UPDATED),
            DataType::RadioPunch => write!(f, "{}", RADIO_PUNCH),
            DataType::CardReadout => write!(f, "{}", CARD_READOUT),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChangeData {
    DropRecord,
    OcChange(OCheckListChange),
    RunUpdateRequest(RunChange),
    RunUpdated(RunChange),
    RadioPunch,
    CardReadout,
}
impl_sqlx_json_text_type_encode_decode!(ChangeData);
#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct ChangesRecord {
    pub id: i64,
    pub source: String,
    pub data_type: DataType,
    pub data_id: Option<DataId>,
    pub data: ChangeData,
    pub user_id: Option<String>,
    pub status: Option<ChangeStatus>,
    pub created: QxDateTime,
    pub lock_number: Option<i64>,
}
#[allow(clippy::too_many_arguments)]
pub async fn add_change(
    event_id: EventId,
    change: ChangesRecord,
    state: &State<SharedQxState>
) -> anyhow::Result<i64> {
    //let change = serde_json::to_value(change).map_err(|e| anyhow!("{e}"))?;
    let edb = get_event_db(event_id, state).await?;
    let id: (i64, ) = query_as("INSERT INTO changes
                (source, data_type, data_id, data, user_id, status, created)
                VALUES (?, ?, ?, ?, ?, ?, ?)  RETURNING id")
        .bind(&change.source)
        .bind(&change.data_type)
        .bind(change.data_id)
        .bind(&change.data)
        .bind(&change.user_id)
        .bind(&change.status)
        .bind(QxDateTime::now().trimmed_to_sec())
        .fetch_one(&edb)
        .await.map_err(sqlx_to_anyhow)?;
    state.read().await.broadcast_change((event_id, change)).await?;
    Ok(id.0)
}

#[get("/event/<event_id>/changes?<from_id>&<limit>")]
async fn get_changes(
    event_id: EventId,
    from_id: Option<i64>,
    limit: Option<i64>,
    session_id: MaybeSessionId,
    state: &State<SharedQxState>,
    gdb: &State<DbPool>
) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, gdb).await?;
    let user = user_info_opt(session_id.0.as_ref(), state).await.map_err(anyhow_to_custom_error)?;
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let records: Vec<ChangesRecord> = sqlx::query_as("SELECT * FROM changes WHERE id>=? ORDER BY created DESC LIMIT ?")
        .bind(from_id.unwrap_or(0))
        .bind(limit.unwrap_or(100))
        .fetch_all(&edb)
        .await
        .map_err(sqlx_to_custom_error)?;
    Ok(Template::render("changes", context! {
            user,
            event,
            records,
        }))
}

#[get("/event/<event_id>/my-changes")]
async fn get_my_changes(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, gdb).await?;
    let user = user_info(&session_id, state).await?;
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let records: Vec<ChangesRecord> = sqlx::query_as("SELECT * FROM changes WHERE user_id=? ORDER BY created")
        .bind(&user.email)
        .fetch_all(&edb)
        .await
        .map_err(sqlx_to_custom_error)?;
    let is_my_changes = true;
    Ok(Template::render("changes", context! {
            is_my_changes,
            user,
            event,
            records,
        }))
}

#[post("/api/event/<event_id>/changes/run-update-request?<data_id>", data = "<data>")]
pub async fn add_run_update_request_change(
    event_id: EventId,
    session_id: QxSessionId,
    data_id: Option<i64>,
    data: Json<RunChange>,
    state: &State<SharedQxState>
) -> Result<Json<i64>, Custom<String>> {
    let user = user_info(&session_id, state).await?;
    let data = ChangeData::RunUpdateRequest(data.into_inner());
    let change_id = add_change(event_id, ChangesRecord {
        id: 0,
        source: "www".to_string(),
        data_type: DataType::RunUpdateRequest,
        data_id,
        data,
        user_id: Some(user.email),
        status: Some(ChangeStatus::Pending),
        created: QxDateTime::now(),
        lock_number: None,
    }, state).await.map_err(anyhow_to_custom_error)?;
    //state.read().await.broadcast_runs_change((event_id, data_id, data)).await.map_err(anyhow_to_custom_error)?;
    Ok(Json(change_id))
}

#[get("/api/event/current/changes/lock-change?<change_id>&<lock_number>")]
async fn api_changes_lock_change(change_id: i64, lock_number: i64, api_token: QxApiToken, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Json<i64>, Custom<String>> {
    let event = load_event_info_for_api_token(&api_token, db).await?;
    let db = get_event_db(event.id, state).await.map_err(anyhow_to_custom_error)?;
    let id: (Option<i64>,) = sqlx::query_as("SELECT lock_number FROM changes WHERE id=?")
        .bind(change_id)
        .bind(lock_number)
        .fetch_one(&db).await.map_err(sqlx_to_custom_error)?;
    if let Some(id) = id.0 {
        Ok(Json(id))
    } else {
        sqlx::query("UPDATE changes SET lock_number=?, status='Locked'  WHERE id=? AND lock_number IS NULL")
            .bind(lock_number)
            .bind(change_id)
            .execute(&db).await.map_err(sqlx_to_custom_error)?;
        Ok(lock_number.into())
    }
}

#[post("/api/event/current/changes/run-updated?<run_id>", data = "<change>")]
async fn add_run_updated_change(run_id: DataId, change: Json<Option<RunChange>>, api_token: QxApiToken, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let event = load_event_info_for_api_token(&api_token, db).await?;
    let run_change = change.into_inner();
    let data = if let Some(run_change) = &run_change {
        ChangeData::RunUpdated(run_change.clone())
    } else {
        ChangeData::DropRecord
    };
    add_change(event.id, ChangesRecord {
        id: 0,
        source: "qe".to_string(),
        data_type: DataType::RunUpdated,
        data_id: Some(run_id),
        data,
        user_id: None,
        status: None,
        created: QxDateTime::now(),
        lock_number: None,
    }, state).await.map_err(anyhow_to_custom_error)?;
    // add_change(event.id, "qe", data_type, run_id, &data, None, None, None, state).await.map_err(anyhow_to_custom_error)?;
    let db = get_event_db(event.id, state).await.map_err(anyhow_to_custom_error)?;
    apply_qe_run_change(run_id, run_change.as_ref(), &db).await.map_err(anyhow_to_custom_error)?;
    Ok(())
}

async fn apply_qe_run_change(run_id: DataId, change: Option<&RunChange>, edb: &SqlitePool) -> anyhow::Result<()> {
    if let Some(change) = change {
        let changed_fields = change.fields_with_value();
        if changed_fields.is_empty() {
            return Err(anyhow!("Cannot apply empty change"));
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
                .bind(run_id)
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
            let placeholders = changed_fields.iter().map(|&fld_name| format!("{fld_name}=?") ).join(",");
            let qs = format!("UPDATE runs SET {placeholders} WHERE run_id=?");
            let mut q = sqlx::query(&qs);
            fn bind_field<'a>(q: Query<'a, Sqlite, SqliteArguments<'a>>, field_name: &'a str, change: &'a RunChange) -> anyhow::Result<Query<'a, Sqlite, SqliteArguments<'a>>> {
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
    } else {
        sqlx::query("DELETE FROM runs WHERE run_id=?")
            .bind(run_id)
            .execute(edb).await.map_err(sqlx_to_anyhow)?;
        Ok(())
    }
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

#[get("/api/event/<event_id>/changes?<from_id>&<limit>&<data_type>&<status>")]
async fn api_changes_get(
    event_id: EventId,
    from_id: Option<i64>,
    limit: Option<i64>,
    data_type: Option<&str>,
    status: Option<&str>,
    state: &State<SharedQxState>
) -> Result<Json<Vec<ChangesRecord>>, Custom<String>> {
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;

    let mut query_builder = QueryBuilder::new("SELECT * FROM changes WHERE id>=");
    query_builder.push_bind(from_id.unwrap_or(0));
    if let Some(data_type) = data_type {
        query_builder.push(" AND data_type=");
        query_builder.push_bind(data_type);
    }
    if let Some(status) = status {
        query_builder.push(format!(" AND status LIKE '{status}%'"));
    }
    query_builder.push(" ORDER BY created");
    if let Some(limit) = limit {
        query_builder.push(" LIMIT ");
        query_builder.push_bind(limit);
    }

    let query = query_builder.build_query_as::<ChangesRecord>();
    let records: Vec<_> = query.fetch_all(&edb).await.map_err(sqlx_to_custom_error)?;
    // info!("records: {:?}", records);
    Ok(records.into())
}

#[delete("/api/event/<event_id>/changes?<change_id>")]
async fn api_changes_delete(
    event_id: EventId,
    change_id: i64,
    session_id: QxSessionId,
    state: &State<SharedQxState>,
) -> Result<(), Custom<String>> {
    let user = user_info(&session_id, state).await?;
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let change: ChangesRecord = sqlx::query_as("SELECT * FROM changes WHERE id=?")
        .bind(change_id)
        .fetch_one(&edb)
        .await
        .map_err(sqlx_to_custom_error)?;
    if let Some(user_id) = change.user_id {
        if user_id == user.email {
            sqlx::query("DELETE FROM changes WHERE id=?")
                .bind(change_id)
                .execute(&edb).await
                .map_err(sqlx_to_custom_error)?;
            return Ok(())
        }
    }
    Err(Custom(Status::Unauthorized, "Only change owner can delete.".into()))
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
        get_changes,
        get_my_changes,
        changes_sse,
        add_run_updated_change,
        add_run_update_request_change,
        api_changes_get,
        api_changes_delete,
        api_changes_lock_change,
    ])
}