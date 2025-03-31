use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use anyhow::anyhow;
use rocket::http::Status;
use rocket::response::status::{Custom};
use rocket::serde::{Deserialize, Serialize};
use rocket::{Build, Rocket, State};
use rocket_dyn_templates::{Template};
use sqlx::{FromRow};
use crate::db::{DbPool};
use crate::{impl_sqlx_json_text_type_encode_decode, QxApiToken, SharedQxState};
use crate::event::{load_event_info, load_event_info_for_api_token, EventId, SiId};
use crate::qxdatetime::QxDateTime;
use crate::util::{anyhow_to_custom_error};
use sqlx::sqlite::SqliteArgumentValue;
use sqlx::{Encode, Sqlite};
use crate::changes::{add_change, ChangeData, DataType};
use crate::qx::QxRunChange;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct OCheckListChangeSet {
    Version: String,
    Creator: String,
    Created: String,
    Event: String,
    Data: Vec<OCheckListChange>,
}

impl_sqlx_json_text_type_encode_decode!(OCheckListChangeSet);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct OCheckListChange {
    pub Runner: OChecklistRunner,
    pub ChangeLog: Option<HashMap<String, String>>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum OChecklistStartStatus {
    #[serde(rename = "Started OK")]
    StartedOk,
    #[serde(rename = "DNS")]
    DidNotStart,
    #[serde(rename = "Late start")]
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
pub(crate) fn load_oc_dir(data_dir: &str) -> anyhow::Result<Vec<OCheckListChangeSet>> {
    info!("Loading test data from: {data_dir}");
    let mut ocs = Vec::new();
    for dir in fs::read_dir(data_dir)? {
        match dir {
            Ok(dir) => {
                match load_oc_file(&dir.path()) {
                    Ok(oc) => { ocs.push(oc) }
                    Err(e) => {
                        error!("Cannot read OC file: {} - {}", dir.path().to_string_lossy(), e.to_string());
                        return Err(anyhow!("{e}"))
                    }
                }
            }
            Err(e) => {
                error!("Cannot read OC dir: {} - {}", data_dir, e.to_string());
                return Err(anyhow!("{e}"))
            }
        }
    }
    Ok(ocs)
}
#[test]
fn test_load_oc() {
    load_oc_dir("tests/oc/data").unwrap();
}

pub(crate) async fn add_oc_change_set(event_id: EventId, change_set: OCheckListChangeSet, state: &State<SharedQxState>) -> anyhow::Result<()> {
    let now = QxDateTime::now();
    let change_dt = QxDateTime::parse_from_string(&change_set.Created, Some(now.0.offset()))?;
    for chng in change_set.Data {
        let Ok(run_chng) = QxRunChange::try_from_oc_change(&chng, Some(change_dt.0.offset())) else {
            continue;
        };
        {
            let data_type = DataType::OcChange;
            let data = ChangeData::OcChange(chng);
            add_change(event_id, "oc", data_type, &data, None, None, state).await?;
        }
        {
            let run_id = run_chng.run_id;
            let data_type = DataType::RunUpdateRequest;
            let data = ChangeData::RunUpdateRequest(run_chng);
            add_change(event_id, "oc", data_type, &data, Some(run_id), None, state).await?;
        }
    }
    Ok(())
}

#[post("/api/event/current/oc", data = "<change_set_yaml>")]
async fn post_oc_change_set(api_token: QxApiToken, change_set_yaml: &str, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let event = load_event_info_for_api_token(&api_token, db).await?;
    let change_set: OCheckListChangeSet = match serde_yaml::from_str(change_set_yaml) {
        Ok(change_set) => {
            change_set
        }
        Err(e) => {
            info!("OC change-set YAML:\n{change_set_yaml}");
            error!("OC change-set YAML parse error: {e}");
            return Err(Custom(Status::InternalServerError, e.to_string()));
        }
    };
    add_oc_change_set(event.id, change_set, state).await.map_err(anyhow_to_custom_error)?;
    Ok(())
}
#[derive(Serialize, FromRow, Clone, Debug)]
struct OCOutRecord {
    id: i64,
    change_set: OCheckListChangeSet,
    created: chrono::DateTime<chrono::Utc>,
}
#[get("/event/<event_id>/oc/out")]
async fn get_oc_out(event_id: EventId, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    // let pool = &db.0;
    // // https://doc.rust-lang.org/rust-by-example/error/iter_result.html
    // let records = sqlx::query_as::<_, OCOutRecord>("SELECT * FROM ocout WHERE event_id=?")
    //     .bind(event_id)
    //     .fetch_all(pool).await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    // let records = records.into_iter()
    //     .flat_map(|r| { 
    //         let created = r.created; 
    //         r.change_set.Data.into_iter().map(move |d| (created, d)) 
    //     }).collect::<Vec<_>>();
    // Ok(Template::render("oc-out", context! {
    //         event,
    //         records,
    //     }))
    debug!("OC out: {}", event.name);
    Err(Custom(Status::InternalServerError, "get_oc_out NIY".to_string()))
}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_oc_out,
            post_oc_change_set,
        ])
}