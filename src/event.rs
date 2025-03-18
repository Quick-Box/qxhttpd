use rocket::serde::json::Json;
use std::io::Cursor;
use anyhow::anyhow;
use base64::engine::general_purpose;
use image::ImageFormat;
use rocket::form::{Contextual, Form};
use rocket::http::{ContentType, Status};
use rocket::response::{Redirect};
use rocket::response::status::Custom;
use rocket::{Build, Data, Rocket, State};
use rocket_dyn_templates::{context, Template};
use sqlx::{query, query_as, FromRow};
use crate::db::DbPool;
use crate::{files, QxApiToken, QxSessionId, SharedQxState};
use crate::auth::{generate_random_string, UserInfo};
use base64::Engine;
use chrono::{DateTime, FixedOffset};
use rocket::serde::{Deserialize, Serialize};
use crate::qe::classes::ClassesRecord;
use crate::qe::import_startlist;
use crate::qe::runs::RunsRecord;
use crate::qxdatetime::QxDateTime;
use crate::util::{anyhow_to_custom_error, sqlx_to_custom_error, string_to_custom_error};

pub const START_LIST_IOFXML3_FILE: &str = "startlist-iof3.xml";

pub type RunId = i64;
pub type SiId = i64;
pub type EventId = i64;

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct Eventrecord {
    pub id: EventId,
    pub name: String,
    pub place: String,
    pub start_time: QxDateTime,
    pub owner: String,
    api_token: QxApiToken,
}
impl Eventrecord {
    pub fn new(owner: &str) -> Self {
        let start_time = QxDateTime::now();
        Self {
            id: 0,
            name: "".to_string(),
            place: "".to_string(),
            start_time,
            // time_zone: "Europe/Prague".to_string(),
            owner: owner.to_string(),
            api_token: QxApiToken(generate_random_string(10)),
        }
    }
}

pub async fn load_event_info(event_id: EventId, db: &State<DbPool>) -> Result<Eventrecord, Custom<String>> {
    let pool = &db.0;
    let event: Eventrecord = sqlx::query_as("SELECT * FROM events WHERE id=?")
        .bind(event_id)
        .fetch_one(pool)
        .await
        .map_err(sqlx_to_custom_error)?;
    Ok(event)
}
pub async fn load_event_info2(qx_api_token: &QxApiToken, db: &State<DbPool>) -> Result<Eventrecord, Custom<String>> {
    let pool = &db.0;
    let event: Eventrecord = sqlx::query_as("SELECT * FROM events WHERE api_token=?")
        .bind(&qx_api_token.0)
        .fetch_one(pool)
        .await
        .map_err(|e| Custom(Status::Unauthorized, e.to_string()))?;
    Ok(event)
}
async fn save_event(event: &Eventrecord, db: &State<DbPool>) -> Result<EventId, anyhow::Error> {
    let id = if event.id > 0 {
        query("UPDATE events SET name=?, place=?, start_time=? WHERE id=?")
            .bind(&event.name)
            .bind(&event.place)
            .bind(event.start_time.0)
            // .bind(&event.time_zone)
            .bind(event.id)
            .execute(&db.0)
            .await.map_err(|e| anyhow!("{e}"))?;
        event.id
    } else {
        let id: (i64, ) = query_as("INSERT INTO events(name, place, start_time, api_token, owner) VALUES (?, ?, ?, ?, ?) RETURNING id")
            .bind(&event.name)
            .bind(&event.place)
            .bind(event.start_time.0)
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
    name: &'v str,
    place: &'v str,
    start_time: &'v str,
    // #[field(validate = len(1..))]
    // owner: &'v str,
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
    let event = Eventrecord {
        id: vals.id,
        name: vals.name.to_string(),
        place: vals.place.to_string(),
        start_time,
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
        Eventrecord::new(&user.email)
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
    for tbl in &["files", "ocout", "qein", "qeout", "runs", "classes" ] {
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
async fn get_event_impl(event_id: EventId, user: Option<UserInfo>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let files = files::list_files(event_id, db).await?;
    Ok(Template::render("event", context! {
        user,
        event,
        files,
    }))
}

#[get("/event/<event_id>", rank = 2)]
async fn get_event(event_id: EventId, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    get_event_impl(event_id, None, db).await
}
#[get("/event/<event_id>")]
async fn get_event_authorized(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    get_event_impl(event_id, Some(user), db).await
}

#[get("/api/event/current")]
async fn get_api_event_current(api_token: QxApiToken, db: &State<DbPool>) -> Result<Json<Eventrecord>, Custom<String>> {
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
async fn post_api_event_current(api_token: QxApiToken, posted_event: Json<PostedEvent>, db: &State<DbPool>) -> Result<Json<Eventrecord>, Custom<String>> {
    let Ok( mut event_info) = load_event_info2(&api_token, db).await else {
        return Err(string_to_custom_error("Event not found"));
    };
    event_info.name = posted_event.name.clone();
    event_info.place = posted_event.place.clone();
    event_info.start_time = posted_event.start_time.into();
    debug!("Post event info, start00: {}", event_info.start_time.to_iso_string());
    let event_id = save_event(&event_info, db).await.map_err(anyhow_to_custom_error)?;
    let reloaded_event = load_event_info(event_id, db).await?;
    Ok(Json(reloaded_event))
}

#[derive(Serialize, Debug)]
struct StartListRecord {
    run: RunsRecord,
    start_time_sec: Option<i64>,
}
#[get("/event/<event_id>/startlist?<class_name>", rank = 2)]
async fn get_event_startlist_anonymous(event_id: EventId, class_name: Option<&str>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    get_event_startlist(event_id, class_name, None, db).await
}
#[get("/event/<event_id>/startlist?<class_name>")]
async fn get_event_startlist_authorized(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, class_name: Option<&str>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    get_event_startlist(event_id, class_name, Some(user), db).await
}
async fn get_event_startlist(event_id: EventId, class_name: Option<&str>, user: Option<UserInfo>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let classes = sqlx::query_as::<_, ClassesRecord>("SELECT * FROM classes WHERE event_id=?")
        .bind(event_id)
        .fetch_all(&db.0).await.map_err(sqlx_to_custom_error)?;
    let class_name = if let Some(name) = class_name {
        name.to_string()
    } else if let Some(classrec) = classes.first() {
        classrec.name.clone()
    } else {
        return Err(Custom(Status::BadRequest, String::from("Classes not found")));
    };
    let classrec = {
        let Some(classrec) = classes.iter().find(|c| c.name == class_name) else {
            return Err(Custom(Status::BadRequest, format!("Class {class_name} not found")));
        };
        classrec.clone()
    };
    let start00 = event.start_time;
    let runs = sqlx::query_as::<_, RunsRecord>("SELECT * FROM runs WHERE event_id=? AND class_name=? ORDER BY start_time")
        .bind(event_id)
        .bind(class_name)
        .fetch_all(&db.0).await.map_err(sqlx_to_custom_error)?;
    let runs = runs.into_iter().map(|run| {
        let start_time_sec = run.start_time.map(|t| t.0.signed_duration_since(start00.0).num_seconds());
        StartListRecord {
            run,
            start_time_sec,
        }
    }).collect::<Vec<_>>();
    Ok(Template::render("startlist", context! {
        user,
        event,
        classrec,
        classes,
        runs,
    }))
}
#[derive(Serialize, Debug)]
struct ResultsRecord {
    run: RunsRecord,
    start_time_sec: Option<i64>,
    finish_time_msec: Option<i64>,
    time_msec: Option<i64>,
}

#[get("/event/<event_id>/results?<class_name>")]
async fn get_event_results(event_id: EventId, class_name: Option<&str>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, db).await?;
    let classes = sqlx::query_as::<_, ClassesRecord>("SELECT * FROM classes WHERE event_id=?")
        .bind(event_id)
        .fetch_all(&db.0).await.map_err(sqlx_to_custom_error)?;
    let class_name = if let Some(name) = class_name {
        name.to_string()
    } else if let Some(classrec) = classes.first() {
        classrec.name.clone()
    } else {
        return Err(Custom(Status::BadRequest, String::from("Classes not found")));
    };
    let classrec = {
        let Some(classrec) = classes.iter().find(|c| c.name == class_name) else {
            return Err(Custom(Status::BadRequest, format!("Class {class_name} not found")));
        };
        classrec.clone()
    };
    let start00 = event.start_time;
    let runs = sqlx::query_as::<_, RunsRecord>("SELECT * FROM runs WHERE event_id=? AND class_name=?")
        .bind(event_id)
        .bind(class_name)
        .fetch_all(&db.0).await.map_err(sqlx_to_custom_error)?;
    let mut runs = runs.into_iter().map(|run| {
        let start_time_sec = run.start_time.map(|t| t.0.signed_duration_since(start00.0).num_seconds());
        let finish_time_msec = run.finish_time.map(|t| t.0.signed_duration_since(start00.0).num_milliseconds());
        let time_msec = run.start_time.and_then(|st| run.finish_time.map(|ft| ft.0.signed_duration_since(st.0).num_milliseconds()));
        ResultsRecord {
            run,
            start_time_sec,
            finish_time_msec,
            time_msec,
        }
    }).collect::<Vec<_>>();
    runs.sort_by_key(|run| if let Some(time) = run.time_msec { time } else { i64::MAX });
    Ok(Template::render("results", context! {
        event,
        classrec,
        classes,
        runs,
    }))

}
#[post("/api/event/current/upload/startlist", data = "<data>")]
async fn post_upload_startlist(qx_api_token: QxApiToken, data: Data<'_>, content_type: &ContentType, db: &State<DbPool>) -> Result<String, Custom<String>> {
    let event_info = load_event_info2(&qx_api_token, db).await?;
    let file_id = crate::files::post_file(qx_api_token, START_LIST_IOFXML3_FILE, data, content_type, db).await?;
    import_startlist(event_info.id, db).await.map_err(anyhow_to_custom_error)?;
    Ok(file_id)
}
#[get("/event/create-demo")]
async fn get_event_create_demo(db: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let mut event_info = Eventrecord::new("fanda.vacek@gmail.com");
    event_info.name = String::from("Demo event");
    event_info.place = String::from("Deep forest 42");
    event_info.api_token = QxApiToken(String::from("plelababamak"));
    let event_id = save_event(&event_info, db).await.map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let data = crate::oc::load_oc_dir("tests/oc/data")
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    for chngset in &data {
        crate::oc::add_oc_change_set(event_id, &event_info.start_time, chngset, db).await.map_err(|e| Custom(Status::InternalServerError, e))?;
    }
    Ok(Redirect::to(format!("/event/{event_id}")))
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_event_create_demo,
            get_event_create,
            get_event_edit,
            get_event_delete,
            post_event,
            get_event,
            get_event_authorized,
            post_upload_startlist,
            get_event_startlist_anonymous,
            get_event_startlist_authorized,
            get_event_results,
            get_api_event_current,
            post_api_event_current,
        ])
}

