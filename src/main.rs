#[macro_use] extern crate rocket;

use std::collections::BTreeMap;
use std::io;
use std::os::linux::raw::stat;
use std::path::{PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::RwLock;
use home::home_dir;
use rocket::data::{Data, ToByteUnit};
use rocket::response::content::RawText;
use serde::Deserialize;
use rocket::State;
use rocket::tokio::fs::{self, File};
use rocket_dyn_templates::{Template, context};

// In a real application, these would be retrieved dynamically from a config.
// const HOST: Absolute<'static> = uri!("http://*:8000");
const DATA_DIR: &'static str = "/tmp/qxhttpd/data";
static RECENT_RECORD_ID: AtomicI32 = AtomicI32::new(0);

type RecordId = i32;
fn next_record_id() -> RecordId {
    let old_id = RECENT_RECORD_ID.fetch_add(1, Ordering::SeqCst);
    old_id + 1
}
fn record_file(id: RecordId) -> io::Result<PathBuf> {
    std::fs::create_dir_all(DATA_DIR)?;
    Ok(PathBuf::from(DATA_DIR).join(format!("{:04}", id)))
}

#[post("/", data = "<paste>")]
async fn upload(paste: Data<'_>) -> io::Result<String> {
    let id = next_record_id();
    let file_path = record_file(id)?;
    println!("file path: {:?}", file_path);
    paste.open(128.kibibytes()).into_file(file_path).await?;
    Ok(id.to_string())
}

#[get("/api/<id>")]
async fn retrieve(id: RecordId) -> Option<RawText<File>> {
    let Ok(file_path) = record_file(id) else { return None };
    File::open(file_path).await.map(RawText).ok()
}

#[delete("/<id>")]
async fn delete(id: RecordId) -> Option<()> {
    let Ok(file_path) = record_file(id) else { return None };
    fs::remove_file(file_path).await.ok()
}

#[get("/")]
fn index() -> Template {
    Template::render("index", context! {
        title: "Quick Event Exchange Server",
    })
}
#[get("/events")]
fn get_events(state: &State<SharedQxState>) -> Template {
    let events = state.read().unwrap().events.keys().map(String::to_owned).collect::<Vec<_>>();
    Template::render("events", context! {
        events: events,
    })
}

struct Event {
    ochecklist_changeset_id_last: usize,
}
struct QxState {
    events: BTreeMap<String, Event>,
}
#[derive(Debug, Deserialize)]
struct AppConfig {
    data_dir: String,
}
type SharedQxState = RwLock<QxState>;
#[launch]
fn rocket() -> _ {
    let data_dir = home_dir().unwrap().join(".qxhttpd/data").as_os_str().to_str().unwrap().to_string();
    let app_config = AppConfig { data_dir };
    let state = QxState {
        events: BTreeMap::from([("test-event".to_string(), Event { ochecklist_changeset_id_last: 0 })])
    };
    rocket::build()
        .attach(Template::fairing())
        .manage(SharedQxState::new(state))
        .manage(app_config)
        .mount("/", routes![index, get_events, upload, delete, retrieve])
}
