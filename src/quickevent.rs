use rocket::serde::{Deserialize, Serialize};
use sqlx::{query, SqlitePool};
use crate::{EventId, RunId, SiId};
use crate::ochecklist::OCheckListChange;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct QERunsChange {
    pub id: u64,
    #[serde(default)]
    pub siId: Option<SiId>,
    #[serde(default)]
    pub checkTime: Option<String>,
    #[serde(default)]
    pub startTime: Option<i64>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub userId: String,
}
impl TryFrom<&OCheckListChange> for QERunsChange {
    type Error = String;

    fn try_from(oc: &OCheckListChange) -> Result<Self, Self::Error> {
        Ok(QERunsChange {
            id: RunId::from_str_radix(&oc.Runner.Id, 10).map_err(|e| e.to_string())?,
            siId: Some(oc.Runner.Card),
            checkTime: Some(oc.Runner.StartTime.clone()),
            startTime: None,
            comment: if oc.Runner.Comment.is_empty() { None } else { Some(oc.Runner.Comment.clone()) },
            source: "oc".to_string(),
            userId: "".to_string(),
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

pub async fn add_qe_in_change_record(event_id: EventId, rec: &QERunsChange, pool: &SqlitePool) {
    let _ = query("INSERT INTO qein 
    (event_id, si_id, start_time, check_time, comment, source, user_id) 
    VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(event_id)
        .bind(rec.siId.map(|n| n as i64))
        .bind(&rec.startTime)
        .bind(&rec.checkTime)
        .bind(&rec.comment)
        .bind(&rec.source)
        .bind(&rec.userId)
        .execute(pool)
        .await.map_err(|e| warn!("Insert QE in record error: {e}"));
}
