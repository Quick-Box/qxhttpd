#[macro_use] extern crate rocket;

use std::collections::BTreeMap;
use std::sync::RwLock;
use home::home_dir;
use rocket::fs::{relative, FileServer};
use rocket::State;
use rocket_dyn_templates::{Template, context};
use log::info;

// In a real application, these would be retrieved dynamically from a config.
// const HOST: Absolute<'static> = uri!("http://*:8000");
type RecordId = i32;
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
    let ochecklist_changeset_id_last = state.read().unwrap().events.get(&event_id).map(|event| event.ochecklist_changeset_id_last).unwrap_or(0);
    Template::render("event", context! {
        ochecklist_changeset_id_last
    })
}

struct Event {
    ochecklist_changeset_id_last: usize,
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
#[launch]
fn rocket() -> _ {
    let state = QxState {
        events: BTreeMap::from([("test-event".to_string(), Event { ochecklist_changeset_id_last: 0 })])
    };
    let rocket = rocket::build()
        .attach(Template::fairing())
        .manage(SharedQxState::new(state))
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
    rocket.manage(cfg)
}
