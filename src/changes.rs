use std::fmt::{Display, Formatter};
use rocket::{Build, Rocket, State};
use rocket::serde::{Deserialize, Serialize};
use sqlx::{query, FromRow};
use crate::event::{EventId, RunId};
use crate::{impl_sqlx_json_text_type_encode_decode, SharedQxState};
use crate::qxdatetime::QxDateTime;
use sqlx::{Encode, Sqlite};
use sqlx::sqlite::SqliteArgumentValue;
use crate::db::{get_event_db};
use crate::oc::OCheckListChange;
use crate::qx::QxRunChange;
use crate::util::{sqlx_to_anyhow};

#[derive(Serialize, Deserialize, Default, Debug)]
pub enum ChangeStatus {
    #[serde(rename = "PND")]
    #[default]
    Pending,
    #[serde(rename = "ACC")]
    Accepted,
    #[serde(rename = "REJ")]
    Rejected,
}
impl_sqlx_json_text_type_encode_decode!(ChangeStatus);

impl Display for ChangeStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeStatus::Pending => f.write_str("PND"),
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
    RunUpdateRequest(QxRunChange),
    RunUpdated(QxRunChange),
    RadioPunch,
    CardReadout,
}
impl_sqlx_json_text_type_encode_decode!(ChangeData);
#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct ChangesRecord {
    pub id: i64,
    pub source: String,
    pub data_type: DataType,
    pub data: ChangeData,
    pub user_id: Option<String>,
    pub run_id: Option<i64>,
    pub status: Option<ChangeStatus>,
    pub created: QxDateTime,
}

pub async fn add_change(event_id: EventId, source: &str, data_type: DataType, data: &ChangeData, run_id: Option<RunId>, user_id: Option<&str>, status: Option<ChangeStatus>, state: &State<SharedQxState>) -> anyhow::Result<()> {
    //let change = serde_json::to_value(change).map_err(|e| anyhow!("{e}"))?;
    let db = get_event_db(event_id, state).await?;
    query("INSERT INTO changes
                (source, data_type, data, run_id, user_id, status, created)
                VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(source)
        .bind(data_type)
        .bind(data)
        .bind(run_id)
        .bind(user_id)
        .bind(status)
        .bind(QxDateTime::now().trimmed_to_sec())
        .execute(&db)
        .await.map_err(sqlx_to_anyhow)?;
    Ok(())
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
    ])
}
