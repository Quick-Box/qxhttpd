#[macro_use] extern crate rocket;

use std::collections::BTreeMap;
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

// In a real application, these would be retrieved dynamically from a config.
// const HOST: Absolute<'static> = uri!("http://*:8000");
type RunId = u64;
type SiId = u64;
type EventId = String;

#[get("/")]
fn index(state: &State<SharedQxState>) -> Template {
    let events = state.read().unwrap().events.keys().map(String::to_owned).collect::<Vec<_>>();
    Template::render("index", context! {
        title: "Quick Event Exchange Server",
        events: events,
    })
}
#[get("/event/<event_id>")]
fn get_event(event_id: EventId, state: &State<SharedQxState>) -> Result<Template, NotFound<String>> {
    let quard = state.read().unwrap();
    let event = quard.events.get(&event_id).ok_or(NotFound(format!("Invalid event ID: {event_id}")))?;
    Ok(Template::render("event", context! {
            event_id,
            event
        }))
}

#[get("/event/<event_id>/qe/chng/in?<offset>&<limit>")]
fn get_in_changes(event_id: EventId, offset: Option<i32>, limit: Option<i32>, state: &State<crate::SharedQxState>) -> Result<Json<Vec<QEChangeRecord>>, String> {
    let quard = state.read().unwrap();
    let event = quard.events.get(&event_id).ok_or(format!("Invalid event ID: {event_id}"))?;
    let mut lst: Vec<QEChangeRecord> = vec![];
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100);
    let mut ix = 0;
    let mut cnt = 0;
    for set in &event.oc.change_sets {
        for rec in &set.Data {
            if ix >= offset && cnt < limit {
                let rec = QERunsRecord::try_from(rec)?;
                lst.push(QEChangeRecord::Runs(rec));
                cnt += 1;
            }
            ix += 1;
        }
    }
    Ok(Json(lst))
}
#[post("/event/<event_id>/oc", data = "<data>")]
async fn add_oc_change_set(event_id: EventId, data: Data<'_>, state: &State<crate::SharedQxState>) -> Result<String, Status> {
    let content = data.open(128.kibibytes()).into_string().await.map_err(|_| Status::InternalServerError)?;
    let oc: OCheckListChangeSet = serde_yaml::from_str(&content).unwrap();
    let mut quard = state.write().unwrap();
    let event = quard.events.get_mut(&event_id).ok_or(Status::NotFound)?;
    event.oc.change_sets.push(oc);
    Ok(format!("{}", event.oc.change_sets.len()))
}
#[derive(Serialize, Clone, Debug)]
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
impl TryFrom<&OCheckListChange> for QERunsRecord {
    type Error = String;

    fn try_from(oc: &OCheckListChange) -> Result<Self, Self::Error> {
        Ok(Self {
            id: RunId::from_str_radix(&oc.Runner.Id, 10).map_err(|e| e.to_string())?,
            siId: oc.Runner.Card,
            checkTime: oc.Runner.StartTime.clone(),
            comment: oc.Runner.Comment.clone(),
        })
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
    oc: OCheckListData,
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
#[derive(Serialize, Clone, Debug)]
struct OCheckListData {
    change_sets: Vec<OCheckListChangeSet>,
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
    events: BTreeMap<EventId, Event>,
}
type SharedQxState = RwLock<QxState>;
fn load_test_oc_data(data_dir: &str) -> Vec<OCheckListChangeSet> {
    fs::read_dir(data_dir).unwrap().map(|dir| {
        let file = dir.unwrap().path();
        info!("loading testing data from file: {:?}", file);
        let content = fs::read_to_string(file).unwrap();
        let oc: OCheckListChangeSet = serde_yaml::from_str(&content).unwrap();
        oc
    }).collect()
}

#[launch]
fn rocket() -> _ {
    let rocket = rocket::build()
        .attach(Template::fairing())
        .mount("/", FileServer::from(relative!("static")))
        .mount("/", routes![
            index,
            get_event,
            get_in_changes,
            add_oc_change_set
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

    let oc_test_data_dir = figment.extract_inner::<String>("qx_oc_test_data_dir");

    let rocket = rocket.manage(cfg);

    let oc_changes = if let Ok(oc_test_data_dir) = oc_test_data_dir {
        load_test_oc_data(&oc_test_data_dir)
    } else { 
        Default::default()
    };
    let state = QxState {
        events: BTreeMap::from([("test-event".to_string(), Event { oc: OCheckListData { change_sets: oc_changes } })])
    };
    let rocket = rocket.manage(SharedQxState::new(state));
    rocket
}

