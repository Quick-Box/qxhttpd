use sqlx::{FromRow};
use rocket::{Build, Data, Rocket, State};
use rocket::data::ToByteUnit;
use rocket::http::{ContentType, Status};
use rocket::response::status::{Custom};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use crate::db::DbPool;
use crate::event::{load_event_info2, EventId};
use crate::{sqlx_error, unzip_data, QxApiToken};

#[derive(Serialize, Deserialize, FromRow)]
pub struct FileInfo {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub created: chrono::DateTime<chrono::Utc>,
}
pub async fn list_files(event_id: EventId, db: &State<DbPool>) -> Result<Vec<FileInfo>, Custom<String>> {
    println!("listing files of event: {event_id}");
    let pool = &db.0;
    let files = sqlx::query_as::<_, FileInfo>("SELECT id, name, LENGTH(data) AS size, created FROM files WHERE event_id=?")
        .bind(event_id)
        .fetch_all(pool).await.map_err(sqlx_error)?;
    Ok(files)
}
#[get("/api/event/<event_id>/file")]
async fn get_files(event_id: EventId, db: &State<DbPool>) -> Result<Json<Vec<FileInfo>>, Custom<String>> {
    let files = list_files(event_id, db).await?;
    Ok(Json(files))
}
#[get("/api/event/<event_id>/file/<file_id>")]
async fn get_file(event_id: EventId, file_id: i64, db: &State<DbPool>) -> Result<Vec<u8>, Custom<String>> {
    let files = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND id=?")
        .bind(event_id)
        .bind(file_id)
        .fetch_one(&db.0).await;
    files.map(|d| d.0 ).map_err(sqlx_error)
}
#[get("/event/<event_id>/file/<file_name>")]
async fn get_file_by_name(event_id: EventId, file_name: &str, db: &State<DbPool>) -> Result<Vec<u8>, Custom<String>> {
    let files = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND name=?")
        .bind(event_id)
        .bind(file_name)
        .fetch_one(&db.0).await;
    files.map(|d| d.0 ).map_err(sqlx_error)
}

#[delete("/api/event/<event_id>/file/<file_id>")]
async fn delete_file(event_id: EventId, file_id: i64, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let res = sqlx::query("DELETE FROM files WHERE event_id=? AND id=?")
        .bind(event_id)
        .bind(file_id)
        .execute(&db.0).await.map_err(sqlx_error)?;
    if res.rows_affected() == 0 {
        Err(Custom(Status::NotFound, format!("File id={file_id} not found")))
    } else {
        Ok(())
    }
}
#[post("/api/event/current/file?<name>", data = "<data>")]
async fn post_file(qx_api_token: QxApiToken, name: &str, data: Data<'_>, content_type: &ContentType, db: &State<DbPool>) -> Result<String, Custom<String>> {
    let event_info = load_event_info2(&qx_api_token, db).await?;
    let data = data.open(50.mebibytes()).into_bytes().await.map_err(|e| Custom(Status::PayloadTooLarge, e.to_string()))?.into_inner();
    let q = sqlx::query_as::<_, (i64,)>("INSERT OR REPLACE INTO files (event_id, name, data) VALUES (?, ?, ?) RETURNING id")
        .bind(event_info.id)
        .bind(name);
    let q = if content_type == &ContentType::ZIP {
        let decompressed = unzip_data(&data).map_err(|e| Custom(Status::UnprocessableEntity, e.to_string()))?;
        info!("Event id: {}, updating file: {name} with {} bytes of data", event_info.id, decompressed.len());
        q.bind(decompressed)
    } else {
        info!("Event id: {}, updating file: {name} with {} bytes of data", event_info.id, data.len());
        q.bind(data)
    };
    let res = q.fetch_one(&db.0).await.map_err(sqlx_error)?;
    Ok(format!("{}", res.0))
}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_files,
            get_file_by_name,
            get_file,
            post_file,
            delete_file,
        ])
}