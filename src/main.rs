#[macro_use] extern crate rocket;

use ::image::ImageFormat;
use std::fmt::Debug;
use std::collections::{HashMap};
use std::io::Cursor;
use std::sync::RwLock;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose;
use rocket::fs::{FileServer};
use rocket::{request, State};
use rocket::form::{Contextual, Form};
use rocket::http::{CookieJar, Status};
use rocket::response::{status, Redirect};
use rocket::response::status::{Custom};
use rocket_dyn_templates::{Template, context, handlebars};
use rocket::serde::Serialize;
use rocket_dyn_templates::handlebars::{Handlebars, Helper};
use serde::{Deserialize};
use sqlx::{query, query_as, FromRow};
use crate::auth::{generate_random_string, UserInfo, QX_SESSION_ID};
use crate::db::{DbPool, DbPoolFairing};

#[cfg(test)]
mod tests;
mod db;
mod auth;

// type Error = String;
// type Error = Box<dyn std::error::Error>;
// type Result<T> = std::result::Result<T, Error>;

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
struct QERadioRecord {
    siId: SiId,
    #[serde(default)]
    time: String,
}

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
struct EventInfo {
    id: EventId,
    name: String,
    place: String,
    date: String,
    api_token: String,
}
// #[derive(Serialize, Deserialize, Clone, Debug)]
// struct Event {
//     info: EventInfo,
//     api_key: String,
// }
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
struct QxSession {
    user_info: UserInfo,
}
#[derive(Eq, Hash, PartialEq)]
struct QxSessionId(String);
#[rocket::async_trait]
impl<'r> request::FromRequest<'r> for QxSessionId {
    type Error = ();
    async fn from_request(request: &'r request::Request<'_>) -> request::Outcome<QxSessionId, ()> {
        let cookies = request
            .guard::<&CookieJar<'_>>()
            .await
            .expect("request cookies");
        if let Some(cookie) = cookies.get_private(QX_SESSION_ID) {
            return request::Outcome::Success(QxSessionId(cookie.value().to_string()));
        }
        request::Outcome::Forward(Status::Unauthorized)
    }
}
struct QxState {
    // events: BTreeMap<EventId, RwLock<EventState>>,
    sessions: HashMap<QxSessionId, QxSession>,
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
async fn index(user: Option<UserInfo>, db: &State<DbPool>) -> std::result::Result<Template, status::Custom<String>> {
    let pool = &db.0;
    let events: Vec<EventInfo> = sqlx::query_as("SELECT * FROM events")
        .fetch_all(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Template::render("index", context! {
            user,
            events,
        }))
}

#[get("/")]
async fn index_authorized(session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> std::result::Result<Template, status::Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    index(Some(user), db).await
}
#[get("/", rank = 2)]
async fn index_anonymous(db: &State<DbPool>) -> std::result::Result<Template, status::Custom<String>> {
    index(None, db).await
}
#[derive(Debug, FromForm)]
// #[allow(dead_code)]
struct EventFormValues<'v> {
    id: EventId,
    #[field(validate = len(1..))]
    name: &'v str,
    #[field(validate = len(1..))]
    place: &'v str,
    #[field(validate = len(1..))]
    date: &'v str,
    api_token: &'v str,
}
// NOTE: We use `Contextual` here because we want to collect all submitted form
// fields to re-render forms with submitted values on error. If you have no such
// need, do not use `Contextual`. Use the equivalent of `Form<Submit<'_>>`.
#[post("/event", data = "<form>")]
async fn post_event<'r>(form: Form<Contextual<'r, EventFormValues<'r>>>, db: &State<DbPool>) -> Result<Redirect, rocket::response::Debug<anyhow::Error>> {
    let vals = form.value.as_ref().ok_or(anyhow::anyhow!("Form data invalid"))?;
    let pool = &db.0;
    if vals.id > 0 {
        query("UPDATE events SET name=?, place=?, date=?, api_token=? WHERE id=?")
            .bind(vals.name.to_string())
            .bind(vals.place.to_string())
            .bind(vals.date.to_string())
            .bind(vals.api_token.to_string())
            .bind(vals.id)
            .execute(pool)
            .await.map_err(|e| anyhow!("{e}"))?;
    } else {
        let id: (i64, ) = query_as("INSERT INTO events(name, place, date, api_token) VALUES (?, ?, ?, ?) RETURNING id")
            .bind(vals.name.to_string())
            .bind(vals.place.to_string())
            .bind(vals.date.to_string())
            .bind(vals.api_token.to_string())
            .fetch_one(pool)
            .await.map_err(|e| anyhow!("{e}"))?;
        info!("Event created, id: {}", id.0);
    };
    Ok(Redirect::to("/"))
}
fn user_info(session_id: QxSessionId, state: &State<SharedQxState>) -> Result<UserInfo, String> {
    state.read().map_err(|e| e.to_string())?
        .sessions.get(&session_id).map(|s| s.user_info.clone()).ok_or("Session expired".to_string() )
}
async fn event_edit_insert(event_id: Option<EventId>, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    let event = if let Some(event_id) = event_id {
        let pool = &db.0;
        let event_info: EventInfo = query_as("SELECT * FROM events WHERE id=?")
            .bind(event_id)
            .fetch_one(pool)
            .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
        event_info
    } else {
        EventInfo {
            id: 0,
            name: "".to_string(),
            place: "".to_string(),
            date: format!("{:?}", chrono::offset::Local::now()),
            api_token: generate_random_string(10),
        }
    };
    let api_token_qrc_img_data = {
        let code = qrcode::QrCode::new(event.api_token.as_bytes()).unwrap();
        // Render the bits into an image.
        let image = code.render::<::image::LumaA<u8>>().build();
        let mut buffer: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        image.write_to(&mut cursor, ImageFormat::Png).unwrap();
        // Encode the image buffer to base64
        general_purpose::STANDARD.encode(&buffer)
    };

    Ok(Template::render("event-edit", context! {
        event_id,
        user,
        event,
        api_token_qrc_img_data,
    }))
}
#[get("/event/create")]
async fn get_event_create(session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    event_edit_insert(None, session_id, state, db).await
}
#[get("/event/edit/<event_id>")]
async fn get_event_edit(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    event_edit_insert(Some(event_id), session_id, state, db).await
}

#[get("/event/<event_id>")]
async fn get_event(event_id: i32, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let pool = &db.0;
    let event: EventInfo = query_as("SELECT * FROM events WHERE id=?")
        .bind(event_id)
        .fetch_one(pool)
        .await.map_err(|e| Custom(Status::NotFound, e.to_string()))?;
    Ok(Template::render("event", context! {
        event,
    }))
}

/*
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
*/
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
        // .attach(Template::fairing())
        .attach(Template::custom(|engines| {
            let handlebars = &mut engines.handlebars;

            // Register a custom Handlebars helper
            handlebars.register_helper("stringify",
                                       Box::new(|h: &Helper, _r: &Handlebars, _: &handlebars::Context, _rc: &mut handlebars::RenderContext, out: &mut dyn handlebars::Output| -> handlebars::HelperResult {
                                           let param = h.param(0).ok_or(handlebars::RenderErrorReason::ParamNotFoundForIndex("stringify", 0))?;
                                           let json = serde_json::to_string(param.value()).unwrap_or_else(|_| "Invalid JSON".to_string());
                                           // out.write("3rd helper: ")?;
                                           // out.write(param.value().render().as_ref())?;
                                           out.write(json.as_ref())?;
                                           Ok(())
                                       }));
        }))
        .attach(DbPoolFairing())
        .mount("/", FileServer::from("./static"))
        .mount("/", routes![
            index_authorized,
            index_anonymous,
            get_event_create,
            get_event_edit,
            post_event,
            get_event,
            // get_oc_changes,
            // get_qe_chng_in,
        ]);
    rocket = auth::extend(rocket);

    // let figment = rocket.figment();
    let cfg = AppConfig::default();

    // let create_demo_event = figment.extract_inner::<bool>("qx_create_demo_event").ok().unwrap_or(false);

    let rocket = rocket.manage(cfg);

    //let load_sample_data = events.is_empty();
    // let e:BTreeMap< crate::EventId, RwLock< crate::Event >> = BTreeMap::from_iter(events);
    let state = QxState {
        sessions: Default::default(),
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


