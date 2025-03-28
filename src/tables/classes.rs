use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct ClassesRecord {
    pub id: i64,
    pub event_id: i64,
    pub name: String,
    pub length: i64,
    pub climb: i64,
    pub control_count: i64,
    pub start_time: Option<DateTime<FixedOffset>>,
    pub interval: i64,
    pub start_slot_count: i64,
}