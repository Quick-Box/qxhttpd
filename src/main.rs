#[macro_use] extern crate rocket;

use std::fmt::Debug;
use std::collections::BTreeMap;
use std::sync::RwLock;
use rocket::fs::{FileServer};
use rocket::{State};
use rocket::form::{Contextual, Form};
use rocket::http::Status;
use rocket::response::status;
use rocket_dyn_templates::{Template, context};
use rocket::serde::Serialize;
use serde::{Deserialize};
use sqlx::{FromRow};
use crate::db::{DbPool, DbPoolFairing};

#[cfg(test)]
mod tests;
mod api;
mod db;

// type Error = String;
type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

// In a real application, these would be retrieved dynamically from a config.
// const HOST: Absolute<'static> = uri!("http://*:8000");
type RunId = u64;
type SiId = u64;
type EventId = i64;
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

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
struct EventInfo {
    id: EventId,
    name: Option<String>,
    place: Option<String>,
    date: Option<chrono::NaiveDateTime>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Event {
    info: EventInfo,
    api_key: api::ApiKey,
}
#[derive(Debug)]
struct EventState {
    event: Event,
}
impl EventState {
}
// fn find_event_by_api_key(data_dir: &str, api_key: &str) -> Result<(Option<EventId>, EventId)> {
//     let mut max_event_id = 0;
//     let mut event_id = None;
//     if let Ok(dirs) = fs::read_dir(data_dir) {
//         for event_dir in dirs {
//             let event_dir = event_dir.map_err(|e| e.to_string())?;
//             let id = usize::from_str(&event_dir.file_name().to_string_lossy()).map_err(|e| e.to_string())?;
//             max_event_id = max(id, max_event_id);
//             let event_info_fn = event_dir.path().join(EVENT_FILE);
//             let event_info: EventInfo = serde_yaml::from_reader(fs::File::open(event_info_fn).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
//             if event_info.api_key == api_key {
//                 assert!(event_id.is_none());
//                 event_id = Some(id);
//             }
//         }
//     }
//     Ok((event_id, max_event_id))
// }

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
}
impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {  }
    }
}
struct QxState {
    events: BTreeMap<EventId, RwLock<EventState>>,
}
impl QxState {
    // async fn create_event(&mut self, mut db: Connection<Db>, mut event_info: EventInfo) -> Result<EventId> {
    //     // NOTE: sqlx#2543, sqlx#1648 mean we can't use the pithier `fetch_one()`.
    //     let results = sqlx::query(
    //         "INSERT INTO events (name, place, date) VALUES (?, ?, ?) RETURNING id"
    //     )
    //         .bind(event_info.name)
    //         .bind(event_info.place)
    //         .bind(event_info.date)
    //         .fetch(&mut **db)
    //         .map_ok(|row| row.get::<u32, _>(0))
    //         .try_collect::<Vec<_>>()
    //         .await?;
    //     let event_id = results.first().expect("returning results");
    //     info!("Creating event id: {}", event_id);
    //     Ok(*event_id as usize)
    // }

    // fn add_oc_change_set(&self, event_id: EventId, change_set: OCheckListChangeSet) -> Result<()> {
    //     let mut event = self.events.get(&event_id).ok_or(format!("Invalid event Id: {event_id}"))?
    //         .write().unwrap();
    //     for rec in &change_set.Data {
    //         let rec = QERunsRecord::try_from(rec).map_err(|e| e.to_string())?;
    //         let _ = event.qe.add_record(&rec);
    //     }
    //     event.oc.add_record(&change_set).map_err(|err| err.to_string())?;
    //     Ok(())
    // }
}
type SharedQxState = RwLock<QxState>;
// fn load_test_oc_data(data_dir: &str) -> Vec<OCheckListChangeSet> {
//     info!("Loading test data from: {data_dir}");
//     fs::read_dir(data_dir).unwrap().map(|dir| {
//         let file = dir.unwrap().path();
//         info!("loading testing data from file: {:?}", file);
//         let content = fs::read_to_string(file).unwrap();
//         let oc: OCheckListChangeSet = serde_yaml::from_str(&content).unwrap();
//         oc
//     }).collect()
// }

#[get("/")]
async fn index(db: &State<DbPool>) -> std::result::Result<Template, status::Custom<String>> {
    let pool = &db.0;

    let events: Vec<EventInfo> = sqlx::query_as("SELECT * FROM events")
        .fetch_all(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Template::render("index", context! {
            events,
        }))
}
#[derive(Debug, FromForm)]
#[allow(dead_code)]
struct SubmitEvent<'v> {
    #[field(validate = len(1..))]
    name: &'v str,
    #[field(validate = len(1..))]
    place: &'v str,
    #[field(validate = len(1..))]
    date: &'v str,
}
// NOTE: We use `Contextual` here because we want to collect all submitted form
// fields to re-render forms with submitted values on error. If you have no such
// need, do not use `Contextual`. Use the equivalent of `Form<Submit<'_>>`.
#[post("/event", data = "<form>")]
async fn create_event<'r>(form: Form<Contextual<'r, SubmitEvent<'r>>>, db: &State<DbPool>) -> (Status, Template) {
    let template = match form.value {
        Some(ref submission) => {
            println!("submission: {:#?}", submission);
            Template::render("success", &form.context)
        }
        None => Template::render("index", &form.context),
    };

    (form.context.status(), template)
}
#[get("/event/<event_id>")]
fn get_event(event_id: EventId, state: &State<SharedQxState>) -> Template {
    let state = state.read().unwrap();
    let event_info = state.events.get(&event_id).expect("event id must exist").read().unwrap().event.info.clone();
    Template::render("event", context! {
            event_id,
            event_info,
        })
}
#[get("/event/<event_id>/qe/chng/in")]
fn get_qe_chng_in(event_id: EventId, state: &State<SharedQxState>) -> Template {
    let state = state.read().unwrap();
    let event_state = state.events.get(&event_id).unwrap().read().unwrap();
    let event_name = &event_state.event.info.name;
    // let change_set = event_state.qe.get_records(0, None).unwrap();
    Template::render("qe-chng-in", context! {
            event_id,
            event_name,
            // change_set
        })
}
#[get("/event/<event_id>/oc")]
fn get_oc_changes(event_id: EventId, state: &State<SharedQxState>) -> Template {
    let state = state.read().unwrap();
    let event_state = state.events.get(&event_id).unwrap().read().unwrap();
    let event_name = &event_state.event.info.name;
    // let change_set = event_state.oc.get_records(0, None).unwrap();
    Template::render("oc-changes", context! {
            event_id,
            event_name,
            // change_set
        })
}

// fn load_event(event_dir: &DirEntry) -> Result<(EventId, Event)> {
//     let event_id = usize::from_str(&event_dir.file_name().to_string_lossy())?;
//     let event: Event = serde_yaml::from_reader(fs::File::open(event_dir.path().join(EVENT_FILE))?)?;
//     //let event_state = EventState {
//     //    event,
//     //    qe: Table::<QERunsRecord>::new(&event_dir.path().join(QECHNGIN_FILE))?,
//     //    oc: Table::<OCheckListChangeSet>::new(&event_dir.path().join(OCCHNGIN_FILE))?,
//     //};
//     Ok((event_id, event))
// }
// fn load_events(state: &mut QxState) -> Result<()> {
//     if let Ok(dirs) = fs::read_dir(&state.data_dir) {
//         for event_dir in dirs {
//             let event_dir = event_dir?;
//             if let Ok((event_id, event)) = load_event(&event_dir) {
//                 let event_dir = event_dir.path();
//                 let event_state = EventState {
//                     event,
//                     qe: Table::<QERunsRecord>::new(&event_dir.join(QECHNGIN_FILE))?,
//                     oc: Table::<OCheckListChangeSet>::new(&event_dir.join(OCCHNGIN_FILE))?,
//                 };
//                 state.events.insert(event_id, RwLock::new(event_state));
//             } else {
//                 error!("Failed to load event: {:?}", event_dir.file_name());
//             }
//         }
//     }
//     Ok(())
// }
// #[get("/users")]
// async fn get_users(db: &State<DbPool>) -> String {
//     let pool = &db.0;
//
//     let users = sqlx::query("SELECT name FROM users")
//         .fetch_all(pool)
//         .await;
//
//     match users {
//         Ok(rows) => {
//             let names: Vec<String> = rows.iter()
//                 .map(|row| row.get::<String, _>("name"))
//                 .collect();
//             format!("Users: {:?}", names)
//         }
//         Err(err) => format!("Error fetching users: {:?}", err),
//     }
// }
#[launch]
fn rocket() -> _ {
    let mut rocket = rocket::build()
        .attach(Template::fairing())
        .attach(DbPoolFairing())
        .mount("/", FileServer::from("./static"))
        .mount("/", routes![
            index,
            create_event,
            get_event,
            get_oc_changes,
            get_qe_chng_in,
        ]);
    rocket = api::mount(rocket);

    // let figment = rocket.figment();
    let cfg = AppConfig::default();

    // let create_demo_event = figment.extract_inner::<bool>("qx_create_demo_event").ok().unwrap_or(false);

    let rocket = rocket.manage(cfg);

    //let load_sample_data = events.is_empty();
    // let e:BTreeMap< crate::EventId, RwLock< crate::Event >> = BTreeMap::from_iter(events);
    let state = QxState {
        events: Default::default(),
    };
    //if create_demo_event {
    //    state.create_event(EventInfo { name: "test-event".to_string(), place: "".to_string(), date: "".to_string() }).unwrap();
    //}
    //else {
    //    load_events(&mut state).unwrap();
    //}
    // if create_demo_event {
    //     let oc_changes = load_test_oc_data("tests/oc/data");
    //     for s in oc_changes {
    //         state.add_oc_change_set(1, s).unwrap();
    //     }
    // }
    rocket.manage(SharedQxState::new(state))
}


