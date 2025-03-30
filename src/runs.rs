use chrono::{DateTime, FixedOffset};
use rocket::response::stream::{Event, EventStream};
use rocket::{Build, Rocket, State};
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use crate::db::{DbPool};
use crate::event::{load_event_info_for_api_token, user_info, EventId};
use crate::qxdatetime::QxDateTime;
use crate::{QxApiToken, QxSessionId, SharedQxState};
use crate::changes::{add_change, ChangeData, DataType};
use crate::qx::QxRunChange;
use crate::util::{anyhow_to_custom_error, sqlx_to_custom_error};

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct ClassesRecord {
    pub id: i64,
    pub event_id: i64,
    pub name: String,
    pub length: i64,
    pub climb: i64,
    pub control_count: i64,
    pub start_time: Option<DateTime<FixedOffset>>,
    pub interval: i64,
    pub start_slot_count: i64,
}


#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct RunsRecord {
    pub id: i64,
    pub event_id: i64,
    pub run_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub class_name: String,
    pub si_id: i64,
    pub registration: String,
    pub start_time: Option<QxDateTime>,
    pub check_time: Option<QxDateTime>,
    pub finish_time: Option<QxDateTime>,
    pub status: String,
    pub edited_by: String,
}
// impl_sqlx_json_text_type_and_decode!(RunsRecord);

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

#[post("/api/event/<event_id>/changes/run-update-request", data = "<change>")]
pub async fn add_run_update_request_change(event_id: EventId, session_id: QxSessionId, change: Json<QxRunChange>, state: &State<SharedQxState>) -> Result<(), Custom<String>> {
    let user = user_info(session_id, state)?;
    let change = change.into_inner();
    let data_type = DataType::RunUpdateRequest;
    let data = ChangeData::RunUpdateRequest(change.clone());
    add_change(event_id, "www", data_type, data, Some(change.run_id), Some(user.email.as_str()), state).await.map_err(anyhow_to_custom_error)?;
    if let Err(e) = state.read().expect("not poisoned")
        .broadcast_runs_change((event_id, change)) {
        error!("Failed to send QE in record error: {e}");
    }

    Ok(())
}

#[post("/api/event/current/changes/run-updated", data = "<change>")]
async fn add_run_updated_change(change: Json<QxRunChange>, api_token: QxApiToken, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let event = load_event_info_for_api_token(&api_token, db).await?;
    let run_change = change.into_inner();
    let run_id = run_change.run_id;
    let data_type = DataType::RunUpdated;
    let data = ChangeData::RunUpdated(run_change);
    add_change(event.id, "qe", data_type, data, Some(run_id), None, state).await.map_err(anyhow_to_custom_error)?;
    Ok(())
}

#[get("/api/event/<event_id>/runs/changes/sse")]
fn runs_changes_sse(event_id: EventId, state: &State<SharedQxState>) -> EventStream![] {
    let mut chng_receiver = state.read().unwrap().runs_changes_sender.subscribe();
    EventStream! {
        loop {
            let (chng_event_id, change) = match chng_receiver.recv().await {
                Ok(chng) => chng,
                Err(e) => {
                    error!("Receive QE in record error: {e}");
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

#[get("/api/event/<event_id>/runs?<run_id>&<class_name>")]
async fn get_runs(event_id: EventId, class_name: Option<&str>, run_id: Option<i32>, db: &State<DbPool>) -> Result<Json<Vec<RunsRecord>>, Custom<String>> {
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
        runs_changes_sse,
        get_runs,
        add_run_updated_change,
        add_run_update_request_change,
    ])
}