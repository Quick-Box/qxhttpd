use chrono::{NaiveDateTime, NaiveTime, TimeDelta};
use rocket::{Build, Rocket};
use rocket::serde::{Deserialize, Serialize};
use crate::oc::OCheckListChange;
use crate::qxdatetime::QxDateTime;

fn is_false(b: &bool) -> bool {
    *b == false
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct QxRunChange {
    pub run_id: i64,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub drop_record: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_id: Option<i64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_time: Option<QxDateTime>,
}
impl QxRunChange {
    pub fn try_from_oc_change(oc: &OCheckListChange, change_set_created_time: QxDateTime) -> anyhow::Result<Self> {
        let mut change = Self {
            run_id: oc.Runner.Id.parse::<i64>()?,
            ..Default::default()
        };
        if let Some(start_time) = &oc.Runner.StartTime {
            // start time can be 10:20:30 or 25-05-01T10:20:03+01:00 depending on version of OCheckList
            change.check_time = if start_time.len() == 8 {
                let tm = NaiveTime::parse_from_str(start_time, "%H:%M:%S")?;
                let dt = change_set_created_time.0.date_naive();
                let dt = NaiveDateTime::new(dt, tm);
                QxDateTime::from_local_timezone(dt, change_set_created_time.0.offset())
            } else {
                QxDateTime::parse_from_string(start_time, Some(&change_set_created_time.0.offset()))?.0
                    // estimate check time to be 2 minutes before start time
                    .checked_sub_signed(TimeDelta::minutes(2))
                    .map(|dt| QxDateTime(dt))
            };
        }
        change.si_id = oc.Runner.NewCard;
        if let Some(change_log) = &oc.ChangeLog {
            if let Some(dtstr) = change_log.get("Late start") {
                // take check time from change log
                let dt = QxDateTime::parse_from_string(dtstr, None)?;
                change. check_time = Some(dt);
            }
            if let Some(_dtstr) = change_log.get("DNS") {
                // no start - no check
                change.check_time = None;
            }
        }
        Ok(change)
    }
    pub fn changed_fields(&self) -> Vec<&str> {
        let mut ret = vec![];
        if self.first_name.is_some() { ret.push("first_name") }
        if self.last_name.is_some() { ret.push("last_name") }
        if self.class_name.is_some() { ret.push("class_name") }
        if self.si_id.is_some() { ret.push("si_id") }
        if self.registration.is_some() { ret.push("registration") }
        if self.start_time.is_some() { ret.push("start_time") }
        if self.check_time.is_some() { ret.push("check_time") }
        if self.finish_time.is_some() { ret.push("finish_time") }
        ret
    }
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
    ])
}


