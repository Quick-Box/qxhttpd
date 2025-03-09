use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct RunsRecord {
    pub id: i64,
    pub event_id: i64,
    pub run_id: i64,
    pub runner_name: String,
    pub class_name: String,
    pub si_id: i64,
    pub registration: String,
    pub start_time: Option<DateTime<FixedOffset>>,
    pub check_time: Option<DateTime<FixedOffset>>,
    pub finish_time: Option<DateTime<FixedOffset>>,
    pub status: String,
    pub edited_by: String,
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
            check_time: Default::default(),
            finish_time: Default::default(),
            status: "".to_string(),
            edited_by: "".to_string(),
        }
    }
}