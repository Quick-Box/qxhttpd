use rocket::serde::{Deserialize, Serialize};
use crate::event::{RunId, SiId};
use crate::qxdatetime::QxDateTime;
use crate::tables::qxchng::{QxChngStatus, QxValChange};

#[derive(Serialize, Deserialize, Debug)]
pub struct RunChange {
    pub run_id: RunId,
    pub property: String,
    pub value: QxValChange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qx_change: Option<(i64, QxChngStatus)>,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum QeChange {
    RunEdit(RunChange),
    RemotePunch(SiId, QxDateTime),
}