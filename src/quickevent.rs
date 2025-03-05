use rocket::http::Status;
use rocket::response::status;
use rocket::response::status::Custom;
use rocket::serde::{Deserialize, Serialize};
use rocket::{Build, Rocket, State};
use rocket::serde::json::Json;
use rocket_dyn_templates::{context, Template};
use sqlx::query;
use crate::{impl_sqlx_json_text_type_and_decode, QxApiToken};
use crate::db::DbPool;
use crate::event::{load_event_info, load_event_info2, EventId, RunId, SiId};
use crate::ochecklist::{OCheckListChange};

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