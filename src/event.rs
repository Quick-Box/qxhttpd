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
use crate::{files, MaybeSessionId, QxApiToken, QxSessionId, SharedQxState};
use crate::auth::{generate_random_string, UserInfo};
use base64::Engine;
use chrono::{DateTime, FixedOffset};
use rocket::serde::{Deserialize, Serialize};
use crate::qxdatetime::QxDateTime;
use crate::tables::classes::ClassesRecord;
use crate::tables::qxchng::{import_runs, import_startlist};
use crate::tables::runs::RunsRecord;
use crate::util::{anyhow_to_custom_error, sqlx_to_custom_error, string_to_custom_error};

pub const START_LIST_IOFXML3_FILE: &str = "startlist-iof3.xml";
pub const RUNS_CSV_FILE: &str = "runs.csv";

pub type RunId = i64;
pub type SiId = i64;
pub type EventId = i64;

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct EventRecord {
    pub id: EventId,
    pub name: String,
    pub place: String,
    pub start_time: QxDateTime,
    pub owner: String,
    api_token: QxApiToken,
}
impl EventRecord {
    pub fn new(owner: &str) -> Self {
        let start_time = QxDateTime::now().trimmed_to_sec();
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

pub async fn load_event_info(event_id: EventId, db: &State<DbPool>) -> Result<EventRecord, Custom<String>> {
    let pool = &db.0;
    let event: EventRecord = sqlx::query_as("SELECT * FROM events WHERE id=?")
        .bind(event_id)
        .fetch_one(pool)
        .await
        .map_err(sqlx_to_custom_error)?;
    Ok(event)
}
pub async fn load_event_info_for_api_token(qx_api_token: &QxApiToken, db: &State<DbPool>) -> Result<EventRecord, Custom<String>> {
    let pool = &db.0;
    let event: EventRecord = sqlx::query_as("SELECT * FROM events WHERE api_token=?")
        .bind(&qx_api_token.0)
        .fetch_one(pool)
        .await
        .map_err(|e| Custom(Status::Unauthorized, e.to_string()))?;
    Ok(event)
}
pub(crate) async fn save_event(event: &EventRecord, db: &State<DbPool>) -> anyhow::Result<EventId> {
    let id = if event.id > 0 {
        query("UPDATE main.events SET name=?, place=?, start_time=? WHERE id=?")
            .bind(&event.name)
            .bind(&event.place)
            .bind(event.start_time.0)
            // .bind(&event.time_zone)
            .bind(event.id)
            .execute(&db.0)
            .await.map_err(|e| anyhow!("{e}"))?;
        event.id
    } else {
        let id: (i64, ) = query_as("INSERT INTO main.events(name, place, start_time, api_token, owner) VALUES (?, ?, ?, ?, ?) RETURNING id")
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
    let user = user_info(session_id, state)?;
    let vals = form.value.as_ref().ok_or(Custom(Status::BadRequest, "Form data invalid".to_string()))?;
    let start_time = QxDateTime::parse_from_iso(vals.start_time)
        .map_err(|e| Custom(Status::BadRequest, format!("Unrecognized date-time string: {}, error: {e}", vals.start_time)))?;
    let event = EventRecord {
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
pub fn user_info(session_id: QxSessionId, state: &State<SharedQxState>) -> Result<UserInfo, Custom<String>> {
    state.read().expect("Not poisoned")
        .sessions.get(&session_id).map(|s| s.user_info.clone()).ok_or( Custom(Status::Unauthorized, "Invalid session ID".to_string()) )
}
pub fn user_info_opt(session_id: MaybeSessionId, state: &State<SharedQxState>) -> anyhow::Result<Option<UserInfo>> {
    match session_id {
        MaybeSessionId::None => Ok(None),
        MaybeSessionId::Some(session_id) => {
            let user_info = state.read().map_err(|e| anyhow!("{e}"))?
                .sessions.get(&session_id).map(|s| s.user_info.clone());
            Ok(user_info)
        }
    }
}
async fn event_edit_insert(event_id: Option<EventId>, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info(session_id, state)?;
    let event = if let Some(event_id) = event_id {
        load_event_info(event_id, db).await?
    } else {
        EventRecord::new(&user.email)
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
        back_link: if let Some(event_id) = event_id {format!("/event/{event_id}")} else {"/".to_string()},
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
    let user = user_info(session_id, state)?;
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

#[get("/event/<event_id>")]
async fn get_event(event_id: EventId, session_id: MaybeSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info_opt(session_id, state).map_err(anyhow_to_custom_error)?;
    get_event_impl(event_id, user, db).await
}

#[get("/api/event/current")]
async fn get_api_event_current(api_token: QxApiToken, db: &State<DbPool>) -> Result<Json<EventRecord>, Custom<String>> {
    let event = load_event_info_for_api_token(&api_token, db).await?;
    Ok(Json(event))
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PostedEvent {
    pub name: String,
    pub place: String,
    pub start_time: DateTime<FixedOffset>,
}
#[post("/api/event/current", data = "<posted_event>")]
async fn post_api_event_current(api_token: QxApiToken, posted_event: Json<PostedEvent>, db: &State<DbPool>) -> Result<Json<EventRecord>, Custom<String>> {
    let Ok( mut event_info) = load_event_info_for_api_token(&api_token, db).await else {
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

#[get("/event/<event_id>/startlist?<class_name>", rank = 2)]
async fn get_event_startlist_anonymous(event_id: EventId, class_name: Option<&str>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    get_event_startlist(event_id, class_name, None, db).await
}
#[get("/event/<event_id>/startlist?<class_name>")]
async fn get_event_startlist_authorized(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, class_name: Option<&str>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info(session_id, state)?;
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
    Ok(Template::render("startlist", context! {
        user,
        event,
        classrec,
        classes,
        runs,
        start00,
    }))
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
    let mut runs = sqlx::query_as::<_, RunsRecord>("SELECT * FROM runs WHERE event_id=? AND class_name=?")
        .bind(event_id)
        .bind(class_name)
        .fetch_all(&db.0).await.map_err(sqlx_to_custom_error)?;
    runs.sort_by_key(|run| {
        let msec = QxDateTime::msec_since_until(&run.start_time, &run.finish_time);
        if let Some(msec) = msec { msec } else { i64::MAX }
    });
    Ok(Template::render("results", context! {
        event,
        classrec,
        classes,
        runs,
        start00,
    }))

}
#[post("/api/event/current/upload/startlist", data = "<data>")]
async fn upload_startlist(qx_api_token: QxApiToken, data: Data<'_>, content_type: &ContentType, db: &State<DbPool>) -> Result<String, Custom<String>> {
    let event_info = load_event_info_for_api_token(&qx_api_token, db).await?;
    let file_id = crate::files::upload_file(qx_api_token, START_LIST_IOFXML3_FILE, data, content_type, db).await?;
    import_startlist(event_info.id, db).await.map_err(anyhow_to_custom_error)?;
    Ok(file_id)
}
#[post("/api/event/current/upload/runs", data = "<data>")]
async fn upload_event(qx_api_token: QxApiToken, data: Data<'_>, content_type: &ContentType, db: &State<DbPool>) -> Result<String, Custom<String>> {
    let event_info = load_event_info_for_api_token(&qx_api_token, db).await?;
    let file_id = crate::files::upload_file(qx_api_token, RUNS_CSV_FILE, data, content_type, db).await?;
    import_runs(event_info.id, db).await.map_err(anyhow_to_custom_error)?;
    Ok(file_id)
}
#[get("/event/create-demo")]
async fn create_demo_event(state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let mut event_info = EventRecord::new("fanda.vacek@gmail.com");
    event_info.name = String::from("Demo event");
    event_info.place = String::from("Deep forest 42");
    event_info.api_token = QxApiToken(String::from("plelababamak"));
    let event_id = save_event(&event_info, db).await.map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

    let data = crate::oc::load_oc_dir("tests/oc/data")
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    for chngset in &data {
        crate::oc::add_oc_change_set(event_id, chngset, state).await.map_err(anyhow_to_custom_error)?;
    }
    Ok(Redirect::to(format!("/event/{event_id}")))
}

#[get("/event/<event_id>/export/runs")]
async fn export_runs(event_id: EventId, db: &State<DbPool>) -> Result<Json<Vec<RunsRecord>>, Custom<String>> {
    let runs = sqlx::query_as::<_, RunsRecord>("SELECT * FROM runs WHERE event_id=?")
        .bind(event_id)
        .fetch_all(&db.0).await.map_err(sqlx_to_custom_error)?;
    Ok(runs.into())
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            create_demo_event,
            get_event_create,
            get_event_edit,
            get_event_delete,
            post_event,
            get_event,
            upload_startlist,
            upload_event,
            get_event_startlist_anonymous,
            get_event_startlist_authorized,
            get_event_results,
            get_api_event_current,
            post_api_event_current,
            export_runs,
        ])
}

