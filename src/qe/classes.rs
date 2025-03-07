use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow, Debug)]
struct ClassesRecord {
    id: i64,
    event_id: i64,
    name: String,
    #[serde(default)]
    length: i64,
    #[serde(default)]
    climb: i64,
    #[serde(default)]
    control_count: i64,
}