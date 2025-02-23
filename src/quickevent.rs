use rocket::http::Status;
use rocket::response::status;
use rocket::response::status::Custom;
use rocket::serde::{Deserialize, Serialize};
use rocket::{Build, Rocket, State};
use rocket_dyn_templates::{context, Template};
use sqlx::{query, FromRow, SqlitePool};
use crate::{EventId, EventInfo, RunId, SiId};
use crate::db::DbPool;
use crate::ochecklist::{OCheckListChange};

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct QERunChange {
    pub run_id: RunId,
    #[serde(default)]
    pub si_id: Option<SiId>,
    #[serde(default)]
    pub check_time: Option<String>,
    #[serde(default)]
    pub start_time: Option<i64>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub user_id: String,
}
impl TryFrom<&OCheckListChange> for QERunChange {
    type Error = String;

    fn try_from(oc: &OCheckListChange) -> Result<Self, Self::Error> {
        Ok(QERunChange {
            run_id: oc.Runner.Id.parse::<i64>().map_err(|e| e.to_string())?,
            si_id: Some(oc.Runner.Card),
            check_time: Some(oc.Runner.StartTime.clone()),
            start_time: None,
            comment: if oc.Runner.Comment.is_empty() { None } else { Some(oc.Runner.Comment.clone()) },
            source: "oc".to_string(),
            user_id: "".to_string(),
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

pub async fn add_qe_in_change_record(event_id: EventId, rec: &QERunChange, pool: &SqlitePool) {
    let _ = query("INSERT INTO qein 
    (event_id, run_id, si_id, start_time, check_time, comment, source, user_id) 
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(event_id)
        .bind(rec.run_id)
        .bind(rec.si_id.map(|n| n as i64))
        .bind(&rec.start_time)
        .bind(&rec.check_time)
        .bind(&rec.comment)
        .bind(&rec.source)
        .bind(&rec.user_id)
        .execute(pool)
        .await.map_err(|e| warn!("Insert QE in record error: {e}"));
}

#[get("/event/<event_id>/qe/in")]
async fn get_qe_in(event_id: EventId, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let pool = &db.0;
    let event: EventInfo = sqlx::query_as("SELECT * FROM events WHERE id=?")
        .bind(event_id)
        .fetch_one(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    let records: Vec<QERunChange> = sqlx::query_as("SELECT * FROM qein WHERE event_id=?")
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
        ])
}