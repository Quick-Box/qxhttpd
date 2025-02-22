use rocket::serde::{Deserialize, Serialize};
use crate::SiId;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
struct OCheckListChangeSet {
    Version: String,
    Creator: String,
    Created: String,
    Event: String,
    Data: Vec<OCheckListChange>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct OCheckListChange {
    pub Runner: OChecklistRunner,
    pub ChangeLog: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum OChecklistStartStatus {
    #[serde(rename = "Started OK")]
    StartedOk,
    DidNotStart,
    LateStart,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct OChecklistRunner {
    pub Id: String,
    pub StartStatus: OChecklistStartStatus,
    pub Card: SiId,
    pub ClassName: String,
    pub Name: String,
    pub StartTime: String,
    #[serde(default)]
    pub Comment: String,
}
