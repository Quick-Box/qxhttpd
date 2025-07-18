use std::collections::HashSet;
use std::fs::OpenOptions;
use rocket::serde::json::Json;
use std::io::{Read};
use anyhow::anyhow;
use rocket::form::{Contextual, Form};
use rocket::http::{Status};
use rocket::response::{Redirect};
use rocket::response::status::Custom;
use rocket::{Build, Rocket, State};
use rocket_dyn_templates::{context, Template};
use sqlx::{query, query_as, FromRow, SqlitePool};
use crate::db::{get_event_db, DbPool};
use crate::{files, MaybeSessionId, QxApiToken, QxSessionId, SharedQxState};
use crate::auth::{generate_random_string, UserInfo};
use chrono::{DateTime, FixedOffset};
use rocket::serde::{Deserialize, Serialize};
use log::info;
use serde_json::Value;
use crate::changes::{ChangesRecord, PENDING, RUN_UPDATE_REQUEST};
use crate::files::{load_file_from_db, save_file_to_db};
use crate::iofxml3::parser::parse_startlist_xml_data;
use crate::qxdatetime::QxDateTime;
use crate::runs::{ClassesRecord, RunsRecord};
use crate::util::{anyhow_to_custom_error, create_qrc, from_csv_json, sqlx_to_anyhow, sqlx_to_custom_error, string_to_custom_error};

pub const START_LIST_IOFXML3_FILE: &str = "startlist-iof3.xml";
pub const RUNS_CSV_JSON_FILE: &str = "runs.csv.json";

// pub type RunId = i64;
pub type SiId = i64;
pub type EventId = i64;

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct EventRecord {
    pub id: EventId,
    pub name: String,
    pub stage: i64,
    pub stage_count: i64,
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
            stage: 1,
            stage_count: 1,
            place: "".to_string(),
            start_time,
            // time_zone: "Europe/Prague".to_string(),
            owner: owner.to_string(),
            api_token: QxApiToken(generate_random_string(10)),
        }
    }
}

pub async fn load_event(event_id: EventId, db: &State<DbPool>) -> anyhow::Result<EventRecord> {
    let pool = &db.0;
    let event: EventRecord = sqlx::query_as("SELECT * FROM events WHERE id=?")
        .bind(event_id)
        .fetch_one(pool)
        .await
        .map_err(sqlx_to_anyhow)?;
    Ok(event)
}
pub async fn load_event_info(event_id: EventId, db: &State<DbPool>) -> Result<EventRecord, Custom<String>> {
    let event = load_event(event_id, db).await.map_err(anyhow_to_custom_error)?;
    Ok(event)
}
pub async fn load_event_info_for_api_token(qx_api_token: &QxApiToken, db: &State<DbPool>) -> Result<EventRecord, Custom<String>> {
    let pool = &db.0;
    let event: EventRecord = sqlx::query_as("SELECT * FROM events WHERE api_token=?")
        .bind(&qx_api_token.0)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            warn!("Unauthorized request for api token: {}, error: {}", qx_api_token.0, e);
            Custom(Status::Unauthorized, e.to_string())
        })?;
    Ok(event)
}
pub(crate) async fn save_event(event: &EventRecord, db: &State<DbPool>) -> anyhow::Result<EventId> {
    let id = if event.id > 0 {
        query("UPDATE events SET name=?, place=?, stage=?, stage_count=?, start_time=? WHERE id=?")
            .bind(&event.name)
            .bind(&event.place)
            .bind(event.stage)
            .bind(event.stage_count)
            .bind(event.start_time.0)
            // .bind(&event.time_zone)
            .bind(event.id)
            .execute(&db.0)
            .await.map_err(|e| anyhow!("{e}"))?;
        event.id
    } else {
        let id: (i64, ) = query_as(
            "INSERT INTO events(name, place, stage, stage_count, start_time, api_token, owner)
                 VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id"
        )
            .bind(&event.name)
            .bind(&event.place)
            .bind(event.stage)
            .bind(event.stage_count)
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
    stage: i64,
    stage_count: i64,
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
    let user = user_info(&session_id, state).await?;
    let vals = form.value.as_ref().ok_or(Custom(Status::BadRequest, "Form data invalid".to_string()))?;
    let start_time = QxDateTime::parse_from_iso(vals.start_time)
        .map_err(|e| Custom(Status::BadRequest, format!("Unrecognized date-time string: {}, error: {e}", vals.start_time)))?;
    let event = if vals.id == 0 {
        EventRecord {
            id: 0,
            name: vals.name.to_string(),
            stage: vals.stage,
            stage_count: vals.stage_count,
            place: vals.place.to_string(),
            start_time,
            owner: user.email.clone(),
            api_token: QxApiToken(vals.api_token.to_string()),
        }
    } else {
        let event = load_event_info(vals.id, db).await?;
        EventRecord {
            name: vals.name.to_string(),
            place: vals.place.to_string(),
            stage: vals.stage,
            stage_count: vals.stage_count,
            start_time,
            ..event
        }
    };
    if event.owner != user.email {
        return Err(Custom(Status::BadRequest, "Different event owner".to_string()))
    }
    let event_id = save_event(&event, db).await.map_err(|e| Custom(Status::BadRequest, e.to_string()))?;
    Ok(Redirect::to(format!("/event/{event_id}")))
}
pub async fn user_info(session_id: &QxSessionId, state: &State<SharedQxState>) -> Result<UserInfo, Custom<String>> {
    state.read().await
        .sessions.get(session_id).map(|s| s.user_info.clone()).ok_or( Custom(Status::Unauthorized, "Invalid session ID".to_string()) )
}
pub async fn user_info_opt(session_id: Option<&QxSessionId>, state: &State<SharedQxState>) -> anyhow::Result<Option<UserInfo>> {
    match &session_id {
        None => Ok(None),
        Some(session_id) => {
            get_user_info(session_id, state).await
        }
    }
}

pub async fn get_user_info(session_id: &QxSessionId, state: &State<SharedQxState>) -> anyhow::Result<Option<UserInfo>> {
    let user_info = state.read().await
        .sessions.get(session_id).map(|s| s.user_info.clone());
    Ok(user_info)
}

pub fn is_event_owner(event: &EventRecord, user: Option<&UserInfo>) -> bool {
    if let Some(user) = user {
        user.email == event.owner
    } else {
        false
    }
}

async fn event_edit_insert(event_id: Option<EventId>, session_id: &QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = get_user_info(session_id, state).await
        .and_then(|u| if let Some(u) = u {Ok(u)} else {Err(anyhow!("Invalid session ID"))})
        .map_err(anyhow_to_custom_error)?;
    let event = if let Some(event_id) = event_id {
        let event = load_event_info(event_id, db).await?;
        if is_event_owner(&event, Some(&user)) {
            event
        } else {
            return Err(Custom(Status::Unauthorized, "Event owner mismatch".to_string()))
        }
    } else {
        EventRecord::new(&user.email)
    };
    let api_token_qrc_img_data = create_qrc(event.api_token.0.as_bytes()).map_err(anyhow_to_custom_error)?;
    Ok(Template::render("event-edit", context! {
        event_id,
        user,
        event,
        api_token_qrc_img_data,
        back_link: if let Some(event_id) = event_id {format!("/event/{event_id}")} else {"/".to_string()},
    }))
}
async fn event_drop(event_id: EventId, db: &State<DbPool>) -> Result<(), anyhow::Error> {
    sqlx::query("DELETE FROM events WHERE id=?")
        .bind(event_id)
        .execute(&db.0).await?;
    //TODO: delete also DB file
    Ok(())
}
#[get("/event/create")]
async fn event_create(session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    event_edit_insert(None, &session_id, state, db).await
}
#[get("/event/<event_id>/edit")]
async fn event_edit(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    event_edit_insert(Some(event_id), &session_id, state, db).await
}
#[get("/event/<event_id>/delete")]
async fn event_delete(event_id: EventId, session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let user = user_info(&session_id, state).await?;
    let event = load_event_info(event_id, db).await?;
    if event.owner == user.email {
        event_drop(event_id, db).await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
        Ok(Redirect::to("/"))
    } else {
        Err(Custom(Status::Unauthorized, String::from("Event owner email mismatch!")))
    }
}

#[get("/event/<event_id>")]
async fn get_event(event_id: EventId, session_id: MaybeSessionId, state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, gdb).await?;
    let user = user_info_opt(session_id.0.as_ref(), state).await.map_err(anyhow_to_custom_error)?;
    let is_event_owner = is_event_owner(&event, user.as_ref());
    let files = files::list_files(event_id, state).await?;
    let is_local_server = state.read().await.app_config.is_local_server();
    let event_url = if is_local_server {
        format!("http://localhost:8000/event/{event_id}")
    } else {
        format!("https://qxqx.org/event/{event_id}")
    };
    let event_qrc_img_data = create_qrc(event_url.as_bytes()).map_err(anyhow_to_custom_error)?;
    Ok(Template::render("event", context! {
        event_url,
        event_qrc_img_data,
        user,
        is_event_owner,
        event,
        files,
    }))
}

#[get("/api/event/current")]
async fn get_api_event_current(api_token: QxApiToken, db: &State<DbPool>) -> Result<Json<EventRecord>, Custom<String>> {
    let event = load_event_info_for_api_token(&api_token, db).await?;
    Ok(Json(event))
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventInfo {
    pub name: String,
    pub stage: i64,
    pub stage_count: i64,
    pub place: String,
    pub start_time: DateTime<FixedOffset>,
    pub classes: Vec<Vec<Value>>,
}
#[post("/api/event/current", data = "<posted_event>")]
async fn post_api_event_current(api_token: QxApiToken, posted_event: Json<EventInfo>, state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<Json<EventRecord>, Custom<String>> {
    let Ok( mut event_info) = load_event_info_for_api_token(&api_token, gdb).await else {
        return Err(string_to_custom_error("Event not found"));
    };
    event_info.name = posted_event.name.clone();
    event_info.stage = posted_event.stage;
    event_info.stage_count = posted_event.stage_count;
    event_info.place = posted_event.place.clone();
    event_info.start_time = posted_event.start_time.into();
    debug!("Post event info, start00: {}", event_info.start_time.to_iso_string());
    let event_id = save_event(&event_info, gdb).await.map_err(anyhow_to_custom_error)?;
    let reloaded_event = load_event_info(event_id, gdb).await?;

    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    import_classes_from_csv_json(posted_event.classes.clone(), &edb).await.map_err(anyhow_to_custom_error)?;

    Ok(Json(reloaded_event))
}

#[get("/event/<event_id>/startlist?<class_name>")]
async fn get_event_start_list(event_id: EventId, session_id: MaybeSessionId, class_name: Option<&str>, state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<Template, Custom<String>> {
    info!("GET session_id: {session_id:?}");
    let event = load_event_info(event_id, gdb).await?;
    let user = user_info_opt(session_id.0.as_ref(), state).await.map_err(anyhow_to_custom_error)?;
    info!("GET user: {user:?}");
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let classes = sqlx::query_as::<_, ClassesRecord>("SELECT * FROM classes ORDER BY name")
        .fetch_all(&edb).await.map_err(sqlx_to_custom_error)?;
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
    let runs = sqlx::query_as::<_, RunsRecord>("SELECT * FROM runs WHERE class_name=? ORDER BY start_time")
        .bind(&class_name)
        .fetch_all(&edb).await.map_err(sqlx_to_custom_error)?;
    let changes = sqlx::query_as::<_, ChangesRecord>("SELECT changes.* FROM changes, runs
                 WHERE runs.class_name=?
                   AND changes.data_id=runs.run_id
                   AND changes.data_type=?
                   AND changes.status=?")
        .bind(&class_name)
        .bind(RUN_UPDATE_REQUEST)
        .bind(PENDING)
        .fetch_all(&edb).await.map_err(sqlx_to_custom_error)?;
    Ok(Template::render("startlist", context! {
        user,
        event,
        classrec,
        classes,
        runs,
        changes,
        start00,
    }))
}

#[get("/event/<event_id>/results?<class_name>")]
async fn get_event_results(event_id: EventId, class_name: Option<&str>, session_id: MaybeSessionId, state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<Template, Custom<String>> {
    let event = load_event_info(event_id, gdb).await?;
    let user = user_info_opt(session_id.0.as_ref(), state).await.map_err(anyhow_to_custom_error)?;
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let classes = sqlx::query_as::<_, ClassesRecord>("SELECT * FROM classes ORDER BY name")
        .fetch_all(&edb).await.map_err(sqlx_to_custom_error)?;
    let class_name = if let Some(name) = class_name {
        name.to_string()
    } else if let Some(classrec) = classes.first() {
        classrec.name.clone()
    } else {
        return Err(Custom(Status::BadRequest, String::from("No classes defined")));
    };
    let classrec = {
        let Some(classrec) = classes.iter().find(|c| c.name == class_name) else {
            return Err(Custom(Status::BadRequest, format!("Class {class_name} not found")));
        };
        classrec.clone()
    };
    let start00 = event.start_time;
    let mut runs = sqlx::query_as::<_, RunsRecord>("SELECT * FROM runs WHERE class_name=?")
        .bind(class_name)
        .fetch_all(&edb).await.map_err(sqlx_to_custom_error)?;
    runs.sort_by_key(|run| {
        let msec = QxDateTime::msec_since_until(&run.start_time, &run.finish_time);
        msec.unwrap_or(i64::MAX)
    });
    Ok(Template::render("results", context! {
        event,
        user,
        classrec,
        classes,
        runs,
        start00,
    }))

}

pub async fn import_start_list(event_id: EventId, edb: &SqlitePool, gdb: &State<DbPool>) -> anyhow::Result<()> {
    let data = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE name=?")
        .bind(START_LIST_IOFXML3_FILE)
        .fetch_one(edb)
        .await.map_err(sqlx_to_anyhow)?.0;

    let (start00, classes, runs) = parse_startlist_xml_data(data).await?;

    sqlx::query("UPDATE events SET start_time=? WHERE id=?")
        .bind(start00)
        .bind(event_id)
        .execute(&gdb.0).await.map_err(sqlx_to_anyhow)?;

    let mut tx = edb.begin().await?;
    for cr in classes {
        sqlx::query("INSERT OR REPLACE INTO classes (name, length, climb, control_count) VALUES (?, ?, ?, ?)")
            .bind(cr.name)
            .bind(cr.length)
            .bind(cr.climb)
            .bind(cr.control_count)
            .execute(&mut *tx).await.map_err(sqlx_to_anyhow)?;
    }
    for run in runs {
        sqlx::query("INSERT OR REPLACE INTO runs (
                             run_id,
                             si_id,
                             last_name,
                             first_name,
                             registration,
                             class_name,
                             start_time,
                             check_time,
                             finish_time
                             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(run.run_id)
            .bind(run.si_id)
            .bind(run.last_name)
            .bind(run.first_name)
            .bind(run.registration)
            .bind(run.class_name)
            .bind(run.start_time.map(|d| d.0))
            .bind(run.check_time.map(|d| d.0))
            .bind(run.finish_time.map(|d| d.0))
            .execute(&mut *tx).await.map_err(sqlx_to_anyhow)?;
    }
    tx.commit().await?;

    Ok(())
}
pub async fn import_classes_from_csv_json(json: Vec<Vec<Value>>, edb: &SqlitePool) -> anyhow::Result<()> {
    let classes: Vec<ClassesRecord> = from_csv_json(json)?;
    let mut tx = edb.begin().await?;

    for cr in classes {
        sqlx::query("INSERT INTO classes (
                            name,
                            length,
                            climb,
                            control_count
                        )
                        VALUES (?, ?, ?, ?)
                        ON CONFLICT(name) DO UPDATE SET
                            length        = excluded.length,
                            climb         = excluded.climb,
                            control_count = excluded.control_count")
            .bind(cr.name)
            .bind(cr.length)
            .bind(cr.climb)
            .bind(cr.control_count)
            .execute(&mut *tx).await.map_err(sqlx_to_anyhow)?;
    }

    tx.commit().await?;
    Ok(())
}
pub async fn import_runs_from_csv_json(json: Vec<Vec<Value>>, edb: &SqlitePool) -> anyhow::Result<()> {
    let runs: Vec<RunsRecord> = from_csv_json(json)?;

    let new_run_ids = runs.iter().map(|run| run.run_id).collect::<HashSet<_>>();
    let curr_run_ids = sqlx::query_as::<_, (i64,)>("SELECT run_id FROM runs")
        .fetch_all(edb)
        .await.map_err(sqlx_to_anyhow)?
        .into_iter().map(|id| id.0).collect::<HashSet<_>>();

    let mut tx = edb.begin().await?;

    for run in runs {
        sqlx::query("INSERT INTO runs (
                             run_id,
                             si_id,
                             last_name,
                             first_name,
                             registration,
                             class_name,
                             start_time,
                             check_time,
                             finish_time
                         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                         ON CONFLICT(run_id) DO UPDATE SET
                            si_id        = excluded.si_id,
                            last_name    = excluded.last_name,
                            first_name   = excluded.first_name,
                            registration = excluded.registration,
                            class_name   = excluded.class_name,
                            start_time   = excluded.start_time,
                            check_time   = excluded.check_time,
                            finish_time  = excluded.finish_time")
            .bind(run.run_id)
            .bind(run.si_id)
            .bind(run.last_name)
            .bind(run.first_name)
            .bind(run.registration)
            .bind(run.class_name)
            .bind(run.start_time.map(|d| d.0))
            .bind(run.check_time.map(|d| d.0))
            .bind(run.finish_time.map(|d| d.0))
            // .bind(run.status)
            .execute(&mut *tx).await.map_err(sqlx_to_anyhow)?;
    }
    for run_id in curr_run_ids.difference(&new_run_ids) {
        sqlx::query("DELETE FROM runs WHERE run_id=?")
            .bind(run_id)
            .execute(&mut *tx).await.map_err(sqlx_to_anyhow)?;
    }

    tx.commit().await?;

    Ok(())
}
pub async fn import_runs_from_db_file(edb: &SqlitePool) -> anyhow::Result<()> {
    let data = load_file_from_db(RUNS_CSV_JSON_FILE, edb).await?;
    let json: Vec<Vec<Value>> = serde_json::from_slice(&data)?;
    import_runs_from_csv_json(json, edb).await
}

pub(crate) const DEMO_API_TOKEN: &str = "plelababamak";
#[cfg(test)]
pub(crate) const TEST_SESSION_ID: &str = "123abc";

#[get("/event/create-demo")]
async fn create_demo_event(state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<Redirect, Custom<String>> {
    let mut event_info = EventRecord::new("fanda.vacek@gmail.com");
    event_info.name = String::from("Demo event");
    event_info.place = String::from("Deep forest 42");
    event_info.api_token = QxApiToken(String::from(DEMO_API_TOKEN));
    let event_id = save_event(&event_info, gdb).await.map_err(|e| Custom(Status::BadRequest, e.to_string()))?;
    {
        // upload demo start list
        let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;

        let mut file = OpenOptions::new().read(true).open(format!("tests/{START_LIST_IOFXML3_FILE}")).unwrap();
        let mut data = vec![];
        file.read_to_end(&mut data).unwrap();
        let _file_id = save_file_to_db(START_LIST_IOFXML3_FILE, &data, &edb).await.map_err(anyhow_to_custom_error)?;
        import_start_list(event_info.id, &edb, gdb).await.map_err(anyhow_to_custom_error)?;
    }
    {
        // upload demo OC changes
        let data = crate::oc::load_oc_dir("tests/oc/data")
            .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
        for chngset in data {
            crate::oc::add_oc_change_set(event_id, chngset, state).await.map_err(anyhow_to_custom_error)?;
        }
    }
    Ok(Redirect::to(format!("/event/{event_id}")))
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            create_demo_event,
            event_create,
            event_edit,
            event_delete,
            post_event,
            get_event,
            get_event_start_list,
            get_event_results,
            get_api_event_current,
            post_api_event_current,
        ])
}

