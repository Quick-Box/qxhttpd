use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow, Debug)]
struct RunsRecord {
    id: i64,
    event_id: i64,
    run_id: i64,
    #[serde(default)]
    runner_name: String,
    #[serde(default)]
    class_name: String,
    #[serde(default)]
    si_id: i64,
    #[serde(default)]
    registration: String,
    #[serde(default)]
    start_time: String,
    #[serde(default)]
    check_time: String,
    #[serde(default)]
    finish_time: String,
    #[serde(default)]
    status: String,
}