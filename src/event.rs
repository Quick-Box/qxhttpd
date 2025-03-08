use rocket::serde::json::Json;
use std::io::Cursor;
use anyhow::anyhow;
use base64::engine::general_purpose;
use image::ImageFormat;
use rocket::form::{Contextual, Form};
use rocket::http::Status;
use rocket::response::{Redirect};
use rocket::response::status::Custom;
use rocket::{Build, Rocket, State};
use rocket_dyn_templates::{context, Template};
use sqlx::{query, query_as, FromRow};
use crate::db::DbPool;
use crate::{files, QxApiToken, QxSessionId, SharedQxState};
use crate::auth::{generate_random_string, UserInfo};
use base64::Engine;
use chrono::{DateTime, FixedOffset};
use rocket::serde::{Deserialize, Serialize};
use crate::qe::classes::ClassesRecord;
use crate::qe::runs::RunsRecord;
use crate::util::{status_sqlx_error, QxDateTime};

pub const START_LIST_IOFXML3_FILE: &str = "startlist-iof3.xml";

pub type RunId = i64;
pub type SiId = i64;
pub type EventId = i64;

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct EventInfo {
    pub id: EventId,
    pub name: String,
    pub place: String,
    pub start_time: DateTime<FixedOffset>,
    pub owner: String,
    api_token: QxApiToken,
}
impl EventInfo {
    pub fn new() -> Self {
        let start_time = chrono::Local::now().fixed_offset();
        Self {
            id: 0,
            name: "".to_string(),
            place: "".to_string(),
            start_time,
            // time_zone: "Europe/Prague".to_string(),
            owner: "".to_string(),
            api_token: QxApiToken(generate_random_string(10)),
        }
    }
}

pub async fn load_event_info(event_id: EventId, db: &State<DbPool>) -> Result<EventInfo, Custom<String>> {
    let pool = &db.0;
    let event: EventInfo = sqlx::query_as("SELECT * FROM events WHERE id=?")
        .bind(event_id)
        .fetch_one(pool)
        .await
        .map_err(status_sqlx_error)?;
    Ok(event)
}
pub async fn load_event_info2(qx_api_token: &QxApiToken, db: &State<DbPool>) -> Result<EventInfo, Custom<String>> {
    let pool = &db.0;
    let event: EventInfo = sqlx::query_as("SELECT * FROM events WHERE api_token=?")
        .bind(&qx_api_token.0)
        .fetch_one(pool)
        .await
        .map_err(|e| Custom(Status::Unauthorized, e.to_string()))?;
    Ok(event)
}
async fn save_event(event: &EventInfo, db: &State<DbPool>) -> Result<EventId, anyhow::Error> {
    let id = if event.id > 0 {
        query("UPDATE events SET name=?, place=?, start_time=? WHERE id=?")
            .bind(&event.name)
            .bind(&event.place)
            .bind(&event.start_time)
            // .bind(&event.time_zone)
            .bind(event.id)
            .execute(&db.0)
            .await.map_err(|e| anyhow!("{e}"))?;
        event.id
    } else {
        let id: (i64, ) = query_as("INSERT INTO events(name, place, start_time, api_token, owner) VALUES (?, ?, ?, ?, ?) RETURNING id")
            .bind(&event.name)
            .bind(&event.place)
            .bind(&event.start_time)
            .bind(&event.api_token.0)
            .bind(&event.owner)
            .fetch_one(&db.0)
            .await.map_err(|e| anyhow!("{e}"))?;
        info!("Event created, id: {}", id.0);
        id.0
    };
    Ok(id)
}
#[derive(Debug, FromForm)]
struct EventFormValues<'v> {
    id: EventId,
    #[field(validate = len(1..))]
    name: &'v str,
    #[field(validate = len(1..))]
    place: &'v str,
    #[field(validate = len(1..))]
    start_time: &'v str,
    // #[field(validate = len(1..))]
    // time_zone: &'v str,
    #[field(validate = len(10..))]
    api_token: &'v str,
}
// NOTE: We use `Contextual` here because we want to collect all submitted form
// fields to re-render forms with submitted values on error. If you have no such
// need, do not use `Contextual`. Use the equivalent of `Form<Submit<'_>>`.
#[post("/event", data = "<form>")]
async fn post_event<'r>(form: Form<Contextual<'r, EventFormValues<'r>>>, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    let vals = form.value.as_ref().ok_or(Custom(Status::BadRequest, "Form data invalid".to_string()))?;
    let start_time = QxDateTime::from_iso_string(vals.start_time)
        .map_err(|e| Custom(Status::BadRequest, format!("Unrecognized date-time string: {}, error: {e}", vals.start_time)))?;
    let event = EventInfo {
        id: vals.id,
        name: vals.name.to_string(),
        place: vals.place.to_string(),
        start_time: start_time.0,
        owner: user.email,
        api_token: QxApiToken(vals.api_token.to_string()),
    };
    save_event(&event, db).await.map_err(|e| Custom(Status::BadRequest, e.to_string()))?;
    Ok(Redirect::to("/"))
}
pub fn user_info(session_id: QxSessionId, state: &State<SharedQxState>) -> Result<UserInfo, String> {
    state.read().map_err(|e| e.to_string())?
        .sessions.get(&session_id).map(|s| s.user_info.clone()).ok_or("Session expired".to_string() )
}
async fn event_edit_insert(event_id: Option<EventId>, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    let event = if let Some(event_id) = event_id {
        load_event_info(event_id, db).await?
    } else {
        EventInfo::new()
    };
    let api_token_qrc_img_data = {
        let code = qrcode::QrCode::new(event.api_token.0.as_bytes()).unwrap();
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
async fn event_drop(event_id: EventId, db: &State<DbPool>) -> Result<(), anyhow::Error> {
    // Start a transaction
    let txn = db.0.begin().await?;
    for tbl in &["files", "ocout", "qein", "qeout", ] {
        sqlx::query(&format!("DELETE FROM {tbl} WHERE event_id=?"))
            .bind(event_id)
            .execute(&db.0).await?;
    }
    sqlx::query("DELETE FROM events WHERE id=?")
        .bind(event_id)
        .execute(&db.0).await?;
    txn.commit().await?;
    Ok(())
}
#[get("/event/create")]
async fn get_event_create(session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    event_edit_insert(None, session_id, state, db).await
}
#[get("/event/<event_id>/edit")]
async fn get_event_edit(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    event_edit_insert(Some(event_id), session_id, state, db).await
}
#[get("/event/<event_id>/delete")]
async fn get_event_delete(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    let event = load_event_info(event_id, db).await?;
    if event.owner == user.email {
        event_drop(event_id, db).await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
        Ok(Redirect::to("/"))
    } else {
        Err(Custom(Status::Unauthorized, String::from("Event owner email mismatch!")))
    }
}

#[get("/event/<event_id>")]
async fn get_event(event_id: EventId, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let files = files::list_files(event_id, db).await?;
    Ok(Template::render("event", context! {
        event,
        files,
    }))
}


#[get("/api/event/current")]
async fn get_api_event_current(api_token: QxApiToken, db: &State<DbPool>) -> Result<Json<EventInfo>, Custom<String>> {
    let event = load_event_info2(&api_token, db).await?;
    Ok(Json(event))
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PostedEvent {
    pub name: String,
    pub place: String,
    pub start_time: DateTime<FixedOffset>,
}
#[post("/api/event/current", data = "<posted_event>")]
async fn post_api_event_current(api_token: QxApiToken, posted_event: Json<PostedEvent>, db: &State<DbPool>) -> Result<Json<EventInfo>, Custom<String>> {
    let Ok( mut event_info) = load_event_info2(&api_token, db).await else {
        return Err(Custom(Status::BadRequest, String::from("Event not found")));
    };
    event_info.name = posted_event.name.clone();
    event_info.place = posted_event.place.clone();
    event_info.start_time = posted_event.start_time;
    let event_id = save_event(&event_info, db).await.map_err(|e| Custom(Status::BadRequest, e.to_string()))?;
    let reloaded_event = load_event_info(event_id, db).await?;
    Ok(Json(reloaded_event))
}
#[get("/event/create-demo")]
async fn get_event_create_demo(db: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let mut event_info = EventInfo::new();
    event_info.name = String::from("Demo event");
    event_info.place = String::from("Deep forest 42");
    event_info.owner = String::from("fanda.vacek@gmail.com");
    event_info.api_token = QxApiToken(String::from("plelababamak"));
    let event_id = save_event(&event_info, db).await.map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let data = crate::oc::load_oc_dir("tests/oc/data")
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    for chngset in &data {
        crate::oc::add_oc_change_set(event_id, chngset, db).await.map_err(|e| Custom(Status::InternalServerError, e))?;
    }
    Ok(Redirect::to(format!("/event/{event_id}")))
}
#[get("/event/<event_id>/startlist?<class_name>")]
async fn get_event_start_list(event_id: EventId, class_name: Option<&str>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let classes = sqlx::query_as::<_, ClassesRecord>("SELECT * FROM classes WHERE event_id=?")
        .bind(event_id)
        .fetch_all(&db.0).await.map_err(status_sqlx_error)?;
    let class_name = if let Some(name) = class_name {
        name.to_string()
    } else {
        if let Some(classrec) = classes.first() {
            classrec.name.clone()
        } else {
            return Err(Custom(Status::BadRequest, String::from("Classes not found")));
        }
    };
    let classrec = {
        let Some(classrec) = classes.iter().find(|c| c.name == class_name) else {
            return Err(Custom(Status::BadRequest, format!("Class {class_name} not found")));
        };
        classrec.clone()
    };
    let start00 = event.start_time;
    let runs = sqlx::query_as::<_, RunsRecord>("SELECT *, 0 as start_time_sec FROM runs WHERE event_id=? AND class_name=? ORDER BY start_time")
        .bind(event_id)
        .bind(class_name)
        .fetch_all(&db.0).await.map_err(status_sqlx_error)?
        .into_iter().map(|mut rec| { rec.start_time_sec = rec.start_time.signed_duration_since(&start00).num_seconds(); rec })
        .collect::<Vec<_>>();
    Ok(Template::render("startlist", context! {
        event,
        classrec,
        classes,
        runs,
    }))

}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_event_create_demo,
            get_event_create,
            get_event_edit,
            get_event_delete,
            post_event,
            get_event,
            get_event_start_list,
            get_api_event_current,
            post_api_event_current,
        ])
}

