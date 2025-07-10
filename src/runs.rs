use chrono::{NaiveDateTime, NaiveTime, TimeDelta};
use qxhttpd_proc_macros::FieldsWithValue;
use rocket::{Build, Rocket, State};
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow};
use crate::db::{get_event_db};
use crate::event::{EventId};
use crate::qxdatetime::QxDateTime;
use crate::{SharedQxState};
use crate::oc::OCheckListChange;
use crate::util::{anyhow_to_custom_error, sqlx_to_custom_error};

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct ClassesRecord {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub length: i64,
    pub climb: i64,
    pub control_count: i64,
    pub start_time: i64,
    pub interval: i64,
    pub start_slot_count: i64,
}

#[allow(dead_code)]
fn is_false(b: &bool) -> bool { !*b }

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[derive(FieldsWithValue)]
pub struct RunChange {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_id: Option<i64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_time: Option<QxDateTime>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_time: Option<QxDateTime>,
    // #[serde(default)]
    // #[serde(skip_serializing_if = "is_false")]
    // pub rent_card: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl RunChange {
    pub fn try_from_oc_change(oc: &OCheckListChange, change_set_created_time: QxDateTime) -> anyhow::Result<(i64, Self)> {
        let run_id = oc.Runner.Id.parse::<i64>()?;
        let mut change = Self::default();
        if let Some(start_time) = &oc.Runner.StartTime {
            // start time can be 10:20:30 or 25-05-01T10:20:03+01:00 depending on version of OCheckList
            change.check_time = if start_time.len() == 8 {
                let tm = NaiveTime::parse_from_str(start_time, "%H:%M:%S")?;
                let dt = change_set_created_time.0.date_naive();
                let dt = NaiveDateTime::new(dt, tm);
                QxDateTime::from_local_timezone(dt, change_set_created_time.0.offset())
            } else {
                QxDateTime::parse_from_string(start_time, Some(change_set_created_time.0.offset()))?.0
                    // estimate check time to be 2 minutes before start time
                    .checked_sub_signed(TimeDelta::minutes(2))
                    .map(QxDateTime)
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
        Ok((run_id, change))
    }
}

#[derive(Serialize, Deserialize, FromRow, Default, Clone, Debug)]
pub struct RunsRecord {
    pub run_id: i64,
    pub class_name: Option<String>,
    #[serde(default)]
    pub registration: Option<String>,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub si_id: Option<i64>,
    #[serde(default)]
    pub start_time: Option<QxDateTime>,
    #[serde(default)]
    pub check_time: Option<QxDateTime>,
    #[serde(default)]
    pub finish_time: Option<QxDateTime>,
}

impl RunsRecord {
}
// #[get("/api/event/<event_id>/runs/changes/sse")]
// async fn runs_changes_sse(event_id: EventId, state: &State<SharedQxState>) -> EventStream![] {
//     let mut chng_receiver = state.read().await.runs_changes_receiver.clone();
//     EventStream! {
//         loop {
//             let (chng_event_id, data_id, change) = match chng_receiver.recv().await {
//                 Ok(chng) => chng,
//                 Err(e) => {
//                     error!("Read run change record error: {e}");
//                     break;
//                 }
//             };
//             if event_id == chng_event_id {
//                 match serde_json::to_string(&change) {
//                     Ok(json) => {
//                         yield Event::data(json);
//                     }
//                     Err(e) => {
//                         error!("Serde error: {e}");
//                         break;
//                     }
//                 }
//             }
//         }
//     }
// }

#[get("/api/event/<event_id>/runs?<run_id>&<class_name>")]
async fn get_runs(event_id: EventId, class_name: Option<&str>, run_id: Option<i32>, state: &State<SharedQxState>) -> Result<Json<Vec<RunsRecord>>, Custom<String>> {
    let db = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let run_id_filter = run_id.map(|id| format!("AND run_id={id}")).unwrap_or_default();
    let class_filter = class_name.map(|n| format!("AND class_name='{n}'")).unwrap_or_default();
    let qs = format!("SELECT * FROM runs WHERE run_id>0 {run_id_filter} {class_filter} ORDER BY run_id");
    let runs = sqlx::query_as::<_, RunsRecord>(&qs)
        .fetch_all(&db).await.map_err(sqlx_to_custom_error)?;
    Ok(runs.into())
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
        // runs_changes_sse,
        get_runs,
    ])
}

#[test]
fn test_fields_with_value() {
    let p = RunChange {
        class_name: Some(String::from("H21")),
        registration: None,
        first_name: None,
        last_name: None,
        si_id: Some(1234),
        start_time: None,
        check_time: None,
        finish_time: Some(QxDateTime::now()),
        note: None,
    };

    let fields = p.fields_with_value();
    assert_eq!(fields, vec!["class_name", "si_id", "finish_time"]);
}