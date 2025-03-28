use rocket::response::stream::{Event, EventStream};
use rocket::{Build, Rocket, State};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow};
use crate::db::{get_event_db};
use crate::event::EventId;
use crate::qxdatetime::QxDateTime;
use crate::SharedQxState;
use crate::tables::qxchng::QxChngStatus;
use crate::util::{sqlx_to_anyhow};

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
pub async fn commit_qe_change(event_id: EventId, change_id: i64, status: QxChngStatus, state: &State<SharedQxState>) -> anyhow::Result<()> {
    let db = get_event_db(event_id, state).await?;
    sqlx::query("UPDATE qxchng SET status=? WHERE id=?")
        .bind(status)
        .bind(change_id)
        .execute(&db).await.map_err(sqlx_to_anyhow)?;
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

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
        runs_changes_sse,
    ])
}