use chrono::{FixedOffset};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct RunsRecord {
    pub id: i64,
    pub event_id: i64,
    pub run_id: i64,
    #[serde(default)]
    pub runner_name: String,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub si_id: i64,
    #[serde(default)]
    pub registration: String,
    #[serde(default)]
    pub start_time: chrono::DateTime<FixedOffset>,
    #[serde(default)]
    #[sqlx(default)]
    pub start_time_sec: i64,
    #[serde(default)]
    pub check_time: String,
    #[serde(default)]
    pub finish_time: String,
    #[serde(default)]
    pub status: String,
}
impl Default for RunsRecord {
    fn default() -> Self {
        Self {
            id: 0,
            event_id: 0,
            run_id: 0,
            runner_name: "".to_string(),
            class_name: "".to_string(),
            si_id: 0,
            registration: "".to_string(),
            start_time: Default::default(),
            start_time_sec: 0,
            check_time: "".to_string(),
            finish_time: "".to_string(),
            status: "".to_string(),
        }
    }
}