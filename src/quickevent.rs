use rocket::serde::{Deserialize, Serialize};
use crate::{RunId, SiId};
use crate::ochecklist::OCheckListChange;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct QERunsRecord {
    pub id: u64,
    #[serde(default)]
    pub siId: SiId,
    #[serde(default)]
    pub checkTime: String,
    #[serde(default)]
    pub comment: String,
}
impl TryFrom<&OCheckListChange> for QERunsRecord {
    type Error = String;

    fn try_from(oc: &OCheckListChange) -> Result<Self, Self::Error> {
        Ok(QERunsRecord {
            id: RunId::from_str_radix(&oc.Runner.Id, 10).map_err(|e| e.to_string())?,
            siId: oc.Runner.Card,
            checkTime: oc.Runner.StartTime.clone(),
            comment: oc.Runner.Comment.clone(),
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
