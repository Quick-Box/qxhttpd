use rocket::serde::{Deserialize, Serialize};
use sqlx::FromRow;
use crate::oc::OCheckListChangeSet;
use crate::qxdatetime::QxDateTime;

#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct OCheckListChngRecord {
    pub id: i64,
    pub change_set: OCheckListChangeSet,
    pub created: QxDateTime,
}