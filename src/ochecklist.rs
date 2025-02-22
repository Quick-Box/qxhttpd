use std::fs;
use rocket::http::Status;
use rocket::response::{status, Redirect};
use rocket::response::status::Custom;
use rocket::serde::{Deserialize, Serialize};
use rocket::{Build, Rocket, State};
use rocket_dyn_templates::{context, Template};
use sqlx::{query, FromRow};
use crate::db::DbPool;
use crate::{EventId, EventInfo, SiId};
use crate::quickevent::{add_qe_in_change_record, QERunsChange};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
struct OCheckListChangeSet {
    Version: String,
    Creator: String,
    Created: String,
    Event: String,
    Data: Vec<OCheckListChange>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct OCheckListChange {
    pub Runner: OChecklistRunner,
    pub ChangeLog: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum OChecklistStartStatus {
    #[serde(rename = "Started OK")]
    StartedOk,
    DidNotStart,
    LateStart,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct OChecklistRunner {
    pub Id: String,
    pub StartStatus: OChecklistStartStatus,
    pub Card: SiId,
    pub ClassName: String,
    pub Name: String,
    pub StartTime: String,
    #[serde(default)]
    pub Comment: String,
}

fn load_test_oc_data(data_dir: &str) -> Vec<OCheckListChangeSet> {
    info!("Loading test data from: {data_dir}");
    fs::read_dir(data_dir).unwrap().map(|dir| {
        let file = dir.unwrap().path();
        info!("loading testing data from file: {:?}", file);
        let content = fs::read_to_string(file).unwrap();
        let oc: OCheckListChangeSet = serde_yaml::from_str(&content).unwrap();
        oc
    }).collect()
}
#[get("/api/event/<event_id>/oc/test/load-data")]
async fn get_load_test_data(event_id: EventId, db: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let data = load_test_oc_data("tests/oc/data");
    let pool = &db.0;
    for set in &data {
        for chng in &set.Data {
            let runner = &chng.Runner;
            query("INSERT INTO ocout
                (event_id, runner_id, start_status, si_id, class_name, runner_name, start_time, comment)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(event_id)
                .bind(runner.Id.parse::<i64>().unwrap_or_default())
                .bind(format!("{:?}",runner.StartStatus))
                .bind(runner.Card as i64)
                .bind(&runner.ClassName)
                .bind(&runner.Name)
                .bind(&runner.StartTime)
                .bind(&runner.Comment)
                .execute(pool)
                .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
            let Ok(qerec) = QERunsChange::try_from(chng) else { continue; };
            add_qe_in_change_record(event_id, &qerec, &pool).await;
        }
    }
    Ok(Redirect::to(format!("/event/{event_id}")))
}
#[derive(Serialize, FromRow, Clone, Debug)]
#[allow(non_snake_case)]
pub struct OCRecord {
    pub runner_id: i64,
    pub start_status: String,
    pub si_id: SiId,
    pub class_name: String,
    pub runner_name: String,
    pub start_time: String,
    #[serde(default)]
    pub comment: String,
}
#[get("/event/<event_id>/oc/out")]
async fn get_oc_out(event_id: EventId, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let pool = &db.0;
    let records: Vec<OCRecord> = sqlx::query_as("SELECT * FROM ocout WHERE event_id=?")
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Template::render("oc-out", context! {
            event_id,
            records,
        }))
}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_oc_out,
            // get_load_test_data,
        ])
}