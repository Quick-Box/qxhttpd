#[macro_use] extern crate rocket;

use std::cmp::min;
use std::fs;
use std::sync::RwLock;
use home::home_dir;
use rocket::fs::{relative, FileServer};
use rocket::{Data, State};
use rocket_dyn_templates::{Template, context};
use log::info;
use rocket::data::ToByteUnit;
use rocket::http::Status;
use rocket::response::status::NotFound;
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use serde::{Deserialize};

#[cfg(test)]
mod tests;

// In a real application, these would be retrieved dynamically from a config.
// const HOST: Absolute<'static> = uri!("http://*:8000");
type RunId = u64;
type SiId = u64;
type EventId = usize;

#[derive(Serialize, Deserialize, Clone, Debug)]
enum QEChangeRecord {
    Runs(QERunsRecord),
    Radio(QERadioRecord),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
struct QERunsRecord {
    id: u64,
    #[serde(default)]
    siId: SiId,
    #[serde(default)]
    checkTime: String,
    #[serde(default)]
    comment: String,
}
impl TryFrom<&OCheckListChange> for QEChangeRecord {
    type Error = String;

    fn try_from(oc: &OCheckListChange) -> Result<Self, Self::Error> {
        Ok(Self::Runs(QERunsRecord {
            id: RunId::from_str_radix(&oc.Runner.Id, 10).map_err(|e| e.to_string())?,
            siId: oc.Runner.Card,
            checkTime: oc.Runner.StartTime.clone(),
            comment: oc.Runner.Comment.clone(),
        }))
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
struct QERadioRecord {
    siId: SiId,
    #[serde(default)]
    time: String,
}

#[derive(Serialize, Clone, Debug)]
struct Event {
    name: String,
    api_key: String,
    qe: Vec<QEChangeRecord>,
    oc: Vec<OCheckListChangeSet>,
}
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
struct OCheckListChange {
    Runner: OChecklistRunner,
    ChangeLog: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
enum OChecklistStartStatus {
    #[serde(rename = "Started OK")]
    StartedOk,
    DidNotStart,
    LateStart,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
struct OChecklistRunner {
    Id: String,
    StartStatus: OChecklistStartStatus,
    Card: SiId,
    ClassName: String,
    Name: String,
    StartTime: String,
    #[serde(default)]
    Comment: String,
}

struct AppConfig {
    data_dir: String,
}
impl Default for AppConfig {
    fn default() -> Self {
        AppConfig { data_dir: "/tmp/qx/data".to_string() }
    }
}
struct QxState {
    events: Vec<Event>,
}
impl QxState {
    fn add_oc_change_set(&mut self, event_id: EventId, change_set: OCheckListChangeSet) -> Result<(), String> {
        let event = self.events.get_mut(event_id).ok_or(format!("Invalid event Id: {event_id}"))?;
        for rec in &change_set.Data {
            let rec = QEChangeRecord::try_from(rec).map_err(|e| e.to_string())?;
            event.qe.push(rec);
        }
        event.oc.push(change_set);
        Ok(())
    }
}
type SharedQxState = RwLock<QxState>;
fn load_test_oc_data(data_dir: &str) -> Vec<OCheckListChangeSet> {
    info!("Loading test data from: {data_dir}");
    fs::read_dir(data_dir).unwrap().map(|dir| {
        let file = dir.unwrap().path();
        info!("loading testing data from file: {:?}", file);
        let content = fs::read_to_string(file).unwrap();
        let oc: OCheckListChangeSet = serde_yaml::from_str(&content).unwrap();
        oc
    }).collect()
}

#[get("/")]
fn index(state: &State<SharedQxState>) -> Template {
    let events = state.read().unwrap().events.iter().enumerate().map(|(ix, event)| (ix, event.name.clone()) ).collect::<Vec<_>>();
    Template::render("index", context! {
        title: "Quick Event Exchange Server",
        events: events,
    })
}
#[get("/event/<event_id>")]
fn get_event(event_id: EventId, state: &State<SharedQxState>) -> Result<Template, NotFound<String>> {
    let quard = state.read().unwrap();
    let event = quard.events.get(event_id).ok_or(NotFound(format!("Invalid event ID: {event_id}")))?;
    Ok(Template::render("event", context! {
            event_id,
            event
        }))
}
#[get("/event/<event_id>/qe/chng/in")]
fn get_qe_chng_in(event_id: EventId, state: &State<crate::SharedQxState>) -> Template {
    let quard = state.read().unwrap();
    let event = quard.events.get(event_id).unwrap();
    let event_name = &event.name;
    let change_set: Vec<&QERunsRecord> = event.qe.iter().filter_map(|rec| {
        if let QEChangeRecord::Runs(rec) = rec {
            Some(rec)
        } else {
            None
        }
    }).collect();
    Template::render("qe-chng-in", context! {
            event_name,
            change_set
        })
}
#[get("/event/<event_id>/oc")]
fn get_oc_changes(event_id: EventId, state: &State<crate::SharedQxState>) -> Template {
    let quard = state.read().unwrap();
    let event = quard.events.get(event_id).unwrap();
    let event_name = &event.name;
    let change_set = &event.oc;
    Template::render("oc-changes", context! {
            event_name,
            change_set
        })
}

#[get("/event/<event_id>/qe/chng/in?<offset>&<limit>")]
fn api_get_in_changes(event_id: EventId, offset: Option<i32>, limit: Option<i32>, state: &State<crate::SharedQxState>) -> Result<Json<Vec<QEChangeRecord>>, String> {
    let quard = state.read().unwrap();
    let event = quard.events.get(event_id).ok_or(format!("Invalid event ID: {event_id}"))?;
    let offset = offset.unwrap_or(0) as usize;
    let offset2 = min(offset + limit.unwrap_or(100) as usize, event.qe.len());
    let lst = event.qe[offset .. offset2].to_vec();
    Ok(Json(lst))
}
#[post("/event/<event_id>/oc", data = "<data>")]
async fn api_add_oc_change_set(event_id: EventId, data: Data<'_>, state: &State<crate::SharedQxState>) -> Result<(), Status> {
    let content = data.open(128.kibibytes()).into_string().await.map_err(|_| Status::InternalServerError)?;
    let oc: OCheckListChangeSet = serde_yaml::from_str(&content).unwrap();
    state.write().unwrap().add_oc_change_set(event_id, oc).map_err(|_| Status::NotFound)?;
    Ok(())
}

#[launch]
fn rocket() -> _ {
    let rocket = rocket::build()
        .attach(Template::fairing())
        .mount("/", FileServer::from(relative!("static")))
        .mount("/", routes![
            index,
            get_event,
            get_oc_changes,
            get_qe_chng_in,
        ])
        .mount("/api/", routes![
            api_get_in_changes,
            api_add_oc_change_set
        ]);

    let figment = rocket.figment();
    let data_dir = figment.extract_inner::<String>("qx_data_dir");
    let mut cfg = AppConfig::default();
    if let Ok(data_dir) = data_dir {
        if !data_dir.is_empty() {
            cfg.data_dir = data_dir;
        }
    }
    if cfg.data_dir.starts_with("~/") {
        let home_dir = home_dir().expect("home dir");
        cfg.data_dir = home_dir.join(&cfg.data_dir[2 ..]).into_os_string().into_string().expect("valid path");
    }
    info!("QX data dir: {}", cfg.data_dir);

    let oc_test_data_dir = if cfg!(test) {
        Some("tests/oc/data".to_string())
    } else {
        figment.extract_inner::<String>("qx_oc_test_data_dir").ok()
    };

    let rocket = rocket.manage(cfg);

    let oc_changes = if let Some(oc_test_data_dir) = oc_test_data_dir {
        load_test_oc_data(&oc_test_data_dir)
    } else { 
        Default::default()
    };
    let mut state = QxState {
        events: vec![
            Event { name: "test-event1".to_string(), api_key: "".to_string(), qe: vec![], oc: vec![] },
            Event { name: "test-event2".to_string(), api_key: "".to_string(), qe: vec![], oc: vec![] },
        ]
    };
    for s in oc_changes {
        state.add_oc_change_set(0, s.clone()).unwrap();
        state.add_oc_change_set(1, s).unwrap();
    }
    let rocket = rocket.manage(SharedQxState::new(state));
    rocket
}

