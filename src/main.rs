#[macro_use] extern crate rocket;

use std::collections::BTreeMap;
use std::fs;
use std::sync::RwLock;
use home::home_dir;
use rocket::fs::{relative, FileServer};
use rocket::State;
use rocket_dyn_templates::{Template, context};
use log::info;
use rocket::serde::Serialize;
use serde::{Deserialize};

// In a real application, these would be retrieved dynamically from a config.
// const HOST: Absolute<'static> = uri!("http://*:8000");
type SiId = u64;
type EventId = String;
//fn record_file(data_dir: &str, id: RecordId) -> io::Result<PathBuf> {
//    std::fs::create_dir_all(data_dir)?;
//    Ok(PathBuf::from(data_dir).join(format!("{:05}", id)))
//}

//#[post("/", data = "<paste>")]
//async fn upload(paste: Data<'_>) -> io::Result<String> {
//    let id = next_record_id();
//    let file_path = record_file(id)?;
//    println!("file path: {:?}", file_path);
//    paste.open(128.kibibytes()).into_file(file_path).await?;
//    Ok(id.to_string())
//}

//#[get("/api/<id>")]
//async fn retrieve(id: RecordId) -> Option<RawText<File>> {
//    let Ok(file_path) = record_file(id) else { return None };
//    File::open(file_path).await.map(RawText).ok()
//}

//#[delete("/<id>")]
//async fn delete(id: RecordId) -> Option<()> {
//    let Ok(file_path) = record_file(id) else { return None };
//    fs::remove_file(file_path).await.ok()
//}

#[get("/")]
fn index(state: &State<SharedQxState>) -> Template {
    let events = state.read().unwrap().events.keys().map(String::to_owned).collect::<Vec<_>>();
    Template::render("index", context! {
        title: "Quick Event Exchange Server",
        events: events,
    })
}
#[get("/event/<event_id>")]
fn get_event(event_id: EventId, state: &State<SharedQxState>) -> Template {
    if let Some(event) = state.read().unwrap().events.get(&event_id) {
        Template::render("event", context! {
            event_id,
            event
        })
    } else {
        Template::render("error/404", context! {})
    }
}

#[derive(Serialize, Clone, Debug)]
struct Event {
    oc: OCheckListData,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct OCheckListChangeSet {
    #[serde(rename(deserialize = "Version"), default)]
    version: String,
    #[serde(rename = "Creator", default)]
    creator: String,
    #[serde(rename(deserialize = "Created"))]
    created: String,
    #[serde(rename = "Event", default)]
    event: String,
    #[serde(rename(deserialize = "Data"), default)]
    data: Vec<OCheckListChange>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct OCheckListChange {
    #[serde(rename(deserialize = "Runner"))]
    runner: OChecklistRunner,
    #[serde(rename = "ChangeLog", default)]
    changelog: String,
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
struct OChecklistRunner {
    #[serde(rename(deserialize = "Id"))]
    id: String,
    #[serde(rename(deserialize = "StartStatus"))]
    start_status: OChecklistStartStatus,
    #[serde(rename(deserialize = "Card"), default)]
    si: SiId,
    #[serde(rename(deserialize = "ClassName"), default)]
    class_name: String,
    #[serde(rename(deserialize = "Name"), default)]
    runner_name: String,
    #[serde(rename(deserialize = "StartTime"), default)]
    start_time: String,
    #[serde(rename(deserialize = "Comment"), default)]
    comment: String,
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
        .mount("/", routes![index, get_event]);

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

