use chrono::{DateTime, FixedOffset};
use rocket::response::stream::{Event, EventStream};
use rocket::{Build, Rocket, State};
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow};
use crate::db::{get_event_db};
use crate::event::{EventId};
use crate::qxdatetime::QxDateTime;
use crate::{SharedQxState};
use crate::util::{anyhow_to_custom_error, sqlx_to_custom_error};

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct ClassesRecord {
    pub id: i64,
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
}
// impl_sqlx_json_text_type_and_decode!(RunsRecord);

impl Default for RunsRecord {
    fn default() -> Self {
        Self {
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
        }
    }
}

#[get("/api/event/<event_id>/runs/changes/sse")]
async fn runs_changes_sse(event_id: EventId, state: &State<SharedQxState>) -> EventStream![] {
    let mut chng_receiver = state.read().await.runs_changes_receiver.clone();
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
async fn get_runs(event_id: EventId, class_name: Option<&str>, run_id: Option<i32>, state: &State<SharedQxState>) -> Result<Json<Vec<RunsRecord>>, Custom<String>> {
    let db = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let run_id_filter = run_id.map(|id| format!("AND run_id={id}")).unwrap_or_default();
    let class_filter = class_name.map(|n| format!("AND class_name='{n}'")).unwrap_or_default();
    let qs = format!("SELECT * FROM runs WHERE run_id>0 {run_id_filter} {class_filter} ORDER BY run_id");
    let runs = sqlx::query_as::<_, RunsRecord>(&qs)
        .fetch_all(&db).await.map_err(sqlx_to_custom_error)?;
    Ok(runs.into())
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
        runs_changes_sse,
        get_runs,
    ])
}