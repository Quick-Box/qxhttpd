use std::fmt::{Display, Formatter};
use anyhow::anyhow;
use rocket::{Build, Rocket, State};
use rocket::response::status::Custom;
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use sqlx::{query, FromRow};
use crate::event::{load_event_info_for_api_token, EventId, RunId, SiId};
use crate::{impl_sqlx_json_text_type_encode_decode, QxApiToken, SharedQxState};
use crate::qxdatetime::QxDateTime;
use sqlx::{Encode, Sqlite};
use sqlx::sqlite::SqliteArgumentValue;
use crate::db::{get_event_db, DbPool};
use crate::oc::OCheckListChange;
use crate::qx::{QxValue, QxValueMap};
use crate::util::{anyhow_to_custom_error, sqlx_to_anyhow};

#[derive(Serialize, Deserialize, Debug)]
pub enum ChangeStatus {
    #[serde(rename = "ACC")]
    Accepted,
    #[serde(rename = "REJ")]
    Rejected,
}
impl_sqlx_json_text_type_encode_decode!(ChangeStatus);

impl Display for ChangeStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeStatus::Accepted => f.write_str("ACC"),
            ChangeStatus::Rejected => f.write_str("REJ"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DataType {
    OcChange,
    RunUpdateRequest,
    RunUpdated,
    RadioPunch,
    CardReadout,
}
impl_sqlx_json_text_type_encode_decode!(DataType);

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

#[derive(Serialize, Deserialize, Debug)]
pub enum ChangeData {
    Null,
    OcChange(OCheckListChange),
    RunUpdateRequest(QxValueMap),
    RunUpdated(QxValueMap),
    RadioPunch,
    CardReadout,
}
impl_sqlx_json_text_type_encode_decode!(ChangeData);
#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct ChangesRecord {
    pub id: i64,
    pub source: String,
    pub data_type: String,
    pub data: ChangeData,
    pub user_id: Option<String>,
    pub run_id: Option<i64>,
    pub status: Option<ChangeStatus>,
    pub created: QxDateTime,
}

pub async fn add_change(event_id: EventId, source: &str, data_type: DataType, data: ChangeData, run_id: Option<RunId>, user_id: Option<&str>, state: &State<SharedQxState>) -> anyhow::Result<()> {
    //let change = serde_json::to_value(change).map_err(|e| anyhow!("{e}"))?;
    let db = get_event_db(event_id, state).await?;
    query("INSERT INTO changes
                (source, data_type, data, run_id, user_id, created)
                VALUES (?, ?, ?, ?, ?, ?)")
        .bind(source)
        .bind(data_type)
        .bind(data)
        .bind(run_id)
        .bind(user_id)
        .bind(QxDateTime::now().trimmed_to_sec())
        .execute(&db)
        .await.map_err(sqlx_to_anyhow)?;
    Ok(())
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
    ])
}
