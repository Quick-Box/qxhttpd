#[macro_use] extern crate rocket;

use std::fmt::Debug;
use std::cmp::max;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::str::FromStr;
use std::sync::RwLock;
use home::home_dir;
use rocket::fs::{FileServer};
use rocket::{Data, State};
use rocket_dyn_templates::{Template, context};
use log::info;
use rocket::data::ToByteUnit;
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use serde::{Deserialize};
use crate::table::Table;

#[cfg(test)]
mod tests;
mod table;

// type Error = String;
type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

// In a real application, these would be retrieved dynamically from a config.
// const HOST: Absolute<'static> = uri!("http://*:8000");
type RunId = u64;
type SiId = u64;
type EventId = usize;
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
    type Error = Error;

    fn try_from(oc: &OCheckListChange) -> Result<Self> {
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
struct QERadioRecord {
    siId: SiId,
    #[serde(default)]
    time: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct EventInfo {
    name: String,
    api_key: String,
}
const EVENT_FILE: &str = "event.yaml";
const OCCHNGIN_FILE: &str = "occhgin.json";
const QECHNGIN_FILE: &str = "qechgin.json";
#[derive(Debug)]
struct Event {
    info: EventInfo,
    qe: Table<QERunsRecord>,
    oc: Table<OCheckListChangeSet>,
}
impl Event {
    fn create(data_dir: &str, event_info: EventInfo) -> Result<(EventId, Event)> {
        let (event_id, max_event_id) = find_event_by_api_key(data_dir, &event_info.api_key)?;
        let event_id = if let Some(event_id) = event_id { event_id } else { max_event_id + 1 };
        let event_dir = Path::new(data_dir).join(format!("{data_dir}/{:0>8}", event_id));
        fs::create_dir_all(&event_dir)?;
        let event_file = event_dir.join(EVENT_FILE);
        info!("Creating event file: {:?}", event_file);
        let f = File::create(event_file)?;
        serde_yaml::to_writer(f, &event_info)?;
        Ok((event_id, Self {
            info: event_info,
            qe: Table::<QERunsRecord>::new(&event_dir.join(QECHNGIN_FILE))?,
            oc: Table::<OCheckListChangeSet>::new(&event_dir.join(OCCHNGIN_FILE))?,
        }))
    }
}
fn find_event_by_api_key(data_dir: &str, api_key: &str) -> Result<(Option<EventId>, EventId)> {
    let mut max_event_id = 0;
    let mut event_id = None;
    if let Ok(dirs) = fs::read_dir(data_dir) {
        for event_dir in dirs {
            let event_dir = event_dir.map_err(|e| e.to_string())?;
            let id = usize::from_str(&event_dir.file_name().to_string_lossy()).map_err(|e| e.to_string())?;
            max_event_id = max(id, max_event_id);
            let event_info_fn = event_dir.path().join(EVENT_FILE);
            let event_info: EventInfo = serde_yaml::from_reader(fs::File::open(event_info_fn).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            if event_info.api_key == api_key {
                assert!(event_id.is_none());
                event_id = Some(id);
            }
        }
    }
    Ok((event_id, max_event_id))
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
        AppConfig {  data_dir: "".to_string()}
    }
}
struct QxState {
    events: BTreeMap<EventId, RwLock<Event>>,
}
impl QxState {
    fn add_oc_change_set(&self, event_id: EventId, change_set: OCheckListChangeSet) -> Result<()> {
        let mut event = self.events.get(&event_id).ok_or(format!("Invalid event Id: {event_id}"))?
            .write().unwrap();
        for rec in &change_set.Data {
            let rec = QERunsRecord::try_from(rec).map_err(|e| e.to_string())?;
            let _ = event.qe.add_record(&rec);
        }
        event.oc.add_record(&change_set).map_err(|err| err.to_string())?;
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
    let events = state.read().unwrap().events.iter()
        .map(|(event_id, event)| (*event_id, event.read().unwrap().info.clone()) ).collect::<Vec<_>>();
    Template::render("index", context! {
        title: "Quick Event Exchange Server",
        events: events,
    })
}
#[get("/event/<event_id>")]
fn get_event(event_id: EventId, state: &State<SharedQxState>) -> Template {
    let state = state.read().unwrap();
    let event = state.events.get(&event_id).unwrap().read().unwrap();
    let event_info = &event.info;
    Template::render("event", context! {
            event_id,
            event_info,
        })
}
#[get("/event/<event_id>/qe/chng/in")]
fn get_qe_chng_in(event_id: EventId, state: &State<crate::SharedQxState>) -> Template {
    let state = state.read().unwrap();
    let event = state.events.get(&event_id).unwrap().read().unwrap();
    let event_name = &event.info.name;
    let change_set = event.qe.get_records(0, None).unwrap();
    Template::render("qe-chng-in", context! {
            event_name,
            change_set
        })
}
#[get("/event/<event_id>/oc")]
fn get_oc_changes(event_id: EventId, state: &State<crate::SharedQxState>) -> Template {
    let state = state.read().unwrap();
    let event = state.events.get(&event_id).unwrap().read().unwrap();
    let event_name = &event.info.name;
    let change_set = event.oc.get_records(0, None).unwrap();
    Template::render("oc-changes", context! {
            event_name,
            change_set
        })
}

#[get("/event/<event_id>/qe/chng/in?<offset>&<limit>")]
fn api_get_qe_in_changes(event_id: EventId, offset: Option<i32>, limit: Option<i32>, state: &State<crate::SharedQxState>) -> Json<Vec<QERunsRecord>> {
    let state = state.read().unwrap();
    let event = state.events.get(&event_id).unwrap().read().unwrap();
    let offset = offset.unwrap_or(0) as usize;
    let lst = event.qe.get_records(offset, limit.map(|l| l as usize)).unwrap();
    Json(lst)
}
#[post("/event/<event_id>/oc", data = "<data>")]
async fn api_add_oc_change_set(event_id: EventId, data: Data<'_>, state: &State<crate::SharedQxState>) -> std::result::Result<(), String> {
    let content = data.open(128.kibibytes()).into_string().await.map_err(|err| err.to_string())?;
    let oc: OCheckListChangeSet = serde_yaml::from_str(&content).unwrap();
    state.write().unwrap().add_oc_change_set(event_id, oc).map_err(|err| err.to_string())?;
    Ok(())
}
const DEFAULT_DATA_DIR: &str = "/tmp/qxhttpd/data";
const TEST_DATA_DIR: &str = "/tmp/test/qxhttpd/data";

fn load_events(data_dir: &str) -> Result<Vec<(EventId, Event)>> {
    let mut ret = Vec::new();
    if let Ok(dirs) = fs::read_dir(data_dir) {
        for event_dir in dirs {
            let event_dir = event_dir?;
            let event_id = usize::from_str(&event_dir.file_name().to_string_lossy())?;
            let event_dir = event_dir.path();
            let event_info: EventInfo = serde_yaml::from_reader(fs::File::open(event_dir.join(EVENT_FILE))?)?;
            let event = Event {
                info: event_info,
                qe: Table::<QERunsRecord>::new(&event_dir.join(QECHNGIN_FILE))?,
                oc: Table::<OCheckListChangeSet>::new(&event_dir.join(OCCHNGIN_FILE))?,
            };
            ret.push((event_id, event));
        }
    }
    Ok(ret)
}

#[launch]
fn rocket() -> _ {
    let rocket = rocket::build()
        .attach(Template::fairing())
        .mount("/", FileServer::from("./static"))
        .mount("/", routes![
            index,
            get_event,
            get_oc_changes,
            get_qe_chng_in,
        ])
        .mount("/api/", routes![
            api_get_qe_in_changes,
            api_add_oc_change_set
        ]);

    let figment = rocket.figment();
    let mut cfg = AppConfig::default();

    let data_dir = if cfg!(test) {
        let _ = fs::remove_dir_all(TEST_DATA_DIR);
        Some(TEST_DATA_DIR.to_string())
    } else {
        figment.extract_inner::<String>("qx_data_dir").ok()
    };
    let data_dir = if let Some(data_dir) = data_dir {
        if data_dir.starts_with("~/") {
            let home_dir = home_dir().expect("home dir");
            home_dir.join(&cfg.data_dir[2 ..]).into_os_string().into_string().expect("valid path")
        } else {
            data_dir
        }
    } else { 
        DEFAULT_DATA_DIR.to_string()
    };
    if data_dir.is_empty() {
        panic!("empty data dir");
    } else {
        cfg.data_dir = data_dir;
    }
    let data_dir = cfg.data_dir.clone();
    info!("QX data dir: {}", data_dir);

    let rocket = rocket.manage(cfg);

    let mut events = load_events(&data_dir).unwrap();
    let load_sample_data = events.is_empty();
    if events.is_empty() {
        let (event_id, event) = Event::create(&data_dir, EventInfo { name: "test-event".to_string(), api_key: "123".to_string() }).unwrap();
        events = vec![
            (event_id, event),
        ];
    }
    // let e:BTreeMap< crate::EventId, RwLock< crate::Event >> = BTreeMap::from_iter(events);
    let state = QxState {
        events: BTreeMap::from_iter(events.into_iter().map(|(event_id, event)| (event_id, RwLock::new(event)))),
    };
    if load_sample_data {
        let oc_changes = load_test_oc_data("tests/oc/data");
        for s in oc_changes {
            state.add_oc_change_set(1, s).unwrap();
        }
    }
    rocket.manage(SharedQxState::new(state))
}

