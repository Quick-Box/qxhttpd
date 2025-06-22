use sqlx::{FromRow, SqlitePool};
use rocket::{Build, Data, Rocket, State};
use rocket::data::ToByteUnit;
use rocket::http::{ContentType, Status};
use rocket::response::status::{Custom};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use crate::db::{get_event_db, DbPool};
use crate::event::{import_runs, import_start_list, load_event_info_for_api_token, EventId, RUNS_CSV_FILE, START_LIST_IOFXML3_FILE};
use crate::{QxApiToken, SharedQxState};
use crate::util::{anyhow_to_custom_error, sqlx_to_anyhow, sqlx_to_custom_error, unzip_data};

#[derive(Serialize, Deserialize, FromRow)]
pub struct FileInfo {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub created: chrono::DateTime<chrono::Utc>,
}
pub async fn list_files(event_id: EventId, state: &State<SharedQxState>) -> Result<Vec<FileInfo>, Custom<String>> {
    println!("listing files of event: {event_id}");
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let files = sqlx::query_as::<_, FileInfo>("SELECT id, name, LENGTH(data) AS size, created FROM files ORDER BY name")
        .fetch_all(&edb).await.map_err(sqlx_to_custom_error)?;
    Ok(files)
}
#[get("/api/event/<event_id>/file")]
async fn get_files(event_id: EventId, state: &State<SharedQxState>) -> Result<Json<Vec<FileInfo>>, Custom<String>> {
    let files = list_files(event_id, state).await?;
    Ok(Json(files))
}
#[get("/api/event/<event_id>/file/<file_id>")]
async fn get_file(event_id: EventId, file_id: i64, state: &State<SharedQxState>) -> Result<Vec<u8>, Custom<String>> {
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let files = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE id=?")
        .bind(file_id)
        .fetch_one(&edb).await;
    files.map(|d| d.0 ).map_err(sqlx_to_custom_error)
}
#[get("/event/<event_id>/file/<file_name>")]
async fn get_file_by_name(event_id: EventId, file_name: &str, state: &State<SharedQxState>) -> Result<Vec<u8>, Custom<String>> {
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let files = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE name=?")
        .bind(file_name)
        .fetch_one(&edb).await;
    files.map(|d| d.0 ).map_err(sqlx_to_custom_error)
}

#[delete("/api/event/<event_id>/file/<file_id>")]
async fn delete_file(event_id: EventId, file_id: i64, state: &State<SharedQxState>) -> Result<(), Custom<String>> {
    let edb = get_event_db(event_id, state).await.map_err(anyhow_to_custom_error)?;
    let res = sqlx::query("DELETE FROM files WHERE id=?")
        .bind(file_id)
        .execute(&edb).await.map_err(sqlx_to_custom_error)?;
    if res.rows_affected() == 0 {
        Err(Custom(Status::NotFound, format!("File id={file_id} not found")))
    } else {
        Ok(())
    }
}

pub(crate) async fn save_file_to_db(name: &str, data: &[u8], edb: &SqlitePool) -> anyhow::Result<i64> {
    let q = sqlx::query_as::<_, (i64,)>("INSERT OR REPLACE INTO files (name, data) VALUES (?, ?) RETURNING id")
        .bind(name)
        .bind(data);
    Ok(q.fetch_one(edb).await.map_err(sqlx_to_anyhow)?.0)
}
pub(crate) async fn load_file_from_db(name: &str, edb: &SqlitePool) -> anyhow::Result<Vec<u8>> {
    let data = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE name=?")
        .bind(name)
        .fetch_one(edb)
        .await.map_err(sqlx_to_anyhow)?.0;
    Ok(data)
}
#[post("/api/event/current/file?<name>", data = "<data>")]
pub async fn upload_file(qx_api_token: QxApiToken, name: &str, data: Data<'_>, content_type: &ContentType, state: &State<SharedQxState>, gdb: &State<DbPool>) -> Result<String, Custom<String>> {
    let event_info = load_event_info_for_api_token(&qx_api_token, gdb).await?;
    let edb = get_event_db(event_info.id, state).await.map_err(anyhow_to_custom_error)?;
    let data = data.open(50.mebibytes()).into_bytes().await.map_err(|e| Custom(Status::PayloadTooLarge, e.to_string()))?.into_inner();
    let data = if content_type == &ContentType::ZIP {
        unzip_data(&data).map_err(|e| Custom(Status::UnprocessableEntity, e.to_string()))?
    } else { 
        data
    };
    let file_id = save_file_to_db(name, &data, &edb).await.map_err(anyhow_to_custom_error)?;
    if name == START_LIST_IOFXML3_FILE {
        import_start_list(event_info.id, &edb, gdb).await.map_err(anyhow_to_custom_error)?;
    }
    else if name == RUNS_CSV_FILE {
        import_runs(&edb).await.map_err(anyhow_to_custom_error)?;
    }
    Ok(format!("{file_id}"))
}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_files,
            get_file_by_name,
            get_file,
            upload_file,
            delete_file,
        ])
}