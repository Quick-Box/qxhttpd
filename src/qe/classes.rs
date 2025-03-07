use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct ClassesRecord {
    pub id: i64,
    pub event_id: i64,
    pub name: String,
    #[serde(default)]
    pub length: i64,
    #[serde(default)]
    pub climb: i64,
    #[serde(default)]
    pub control_count: i64,
}