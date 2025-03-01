use crate::quickevent::add_qe_in_change_record;
use std::fs;
use std::path::PathBuf;
use rocket::http::Status;
use rocket::response::{Redirect};
use rocket::response::status::{Created, Custom};
use rocket::serde::{Deserialize, Serialize};
use rocket::{Build, Rocket, State};
use rocket_dyn_templates::{context, Template};
use sqlx::{query, FromRow};
use crate::db::DbPool;
use crate::{EventId, SiId};
use crate::event::load_event_info;
use crate::quickevent::{QERunChange};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
struct OCheckListChangeSet {
    Version: String,
    Creator: String,
    Created: String,
    Event: String,
    Data: Vec<OCheckListChange>,
}
impl sqlx::Type<sqlx::Sqlite> for OCheckListChangeSet
{
    fn type_info() -> <sqlx::Sqlite as sqlx::Database>::TypeInfo {
        <&str as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}
impl<'r, DB: sqlx::Database> sqlx::Decode<'r, DB> for OCheckListChangeSet
where &'r str: sqlx::Decode<'r, DB>
{
    fn decode(value: <DB as sqlx::Database>::ValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <&str as sqlx::Decode<DB>>::decode(value)?;
        Ok(serde_json::from_str::<OCheckListChangeSet>(value)?)
    }
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

fn load_oc_change_set(content: &str) -> anyhow::Result<OCheckListChangeSet> {
    Ok(serde_yaml::from_str(content)?)
}
fn load_oc_file(file: &PathBuf) -> anyhow::Result<OCheckListChangeSet> {
    info!("Loading OCheckList change set from file: {}", file.to_string_lossy());
    let content = fs::read_to_string(file)?;
    load_oc_change_set(&content)
}
fn load_oc_dir(data_dir: &str) -> anyhow::Result<Vec<OCheckListChangeSet>> {
    info!("Loading test data from: {data_dir}");
    let ocs = fs::read_dir(data_dir)?.map(|dir| {
        match dir {
            Ok(dir) => {
                match load_oc_file(&dir.path()) {
                    Ok(oc) => { Some(oc) }
                    Err(e) => {
                        error!("Cannot read OC file: {} - {}", dir.path().to_string_lossy(), e.to_string());
                        None
                    }
                }
            }
            Err(e) => {
                error!("Cannot read OC dir: {} - {}", data_dir, e.to_string());
                None
            }
        }
    })
        .filter_map(|oc| oc)
        .collect();
    Ok(ocs)
}
#[get("/api/event/<event_id>/oc/test/load-data")]
async fn get_load_test_data(event_id: EventId, db: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let data = load_oc_dir("tests/oc/data")
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    let pool = &db.0;
    for chngset in &data {
        query("INSERT INTO ocout
                (event_id, change_set)
                VALUES (?, ?)")
            .bind(event_id)
            .bind(serde_json::to_string(chngset).map_err(|e| Custom(Status::InternalServerError, e.to_string()))?)
            .execute(pool)
            .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
        for chng in &chngset.Data {
            let Ok(qerec) = QERunChange::try_from(chng) else { continue; };
            add_qe_in_change_record(event_id, "oc", None, &qerec, &pool).await;
        }
    }
    Ok(Redirect::to(format!("/event/{event_id}")))
}
#[derive(Serialize, FromRow, Clone, Debug)]
struct OCOutRecord {
    id: i64,
    event_id: i64,
    change_set: OCheckListChangeSet,
    created: String
}
#[get("/event/<event_id>/oc/out")]
async fn get_oc_out(event_id: EventId, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let pool = &db.0;
    // https://doc.rust-lang.org/rust-by-example/error/iter_result.html
    let records = sqlx::query_as::<_, OCOutRecord>("SELECT * FROM ocout WHERE event_id=?")
        .bind(event_id)
        .fetch_all(pool).await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    let records = records.into_iter()
        .map(|r| { 
            let created = r.created; r.change_set.Data.into_iter().map(move |d| (created.clone(), d)) 
        }).flatten().collect::<Vec<_>>();
    Ok(Template::render("oc-out", context! {
            event,
            records,
        }))
}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_oc_out,
            get_load_test_data,
        ])
}