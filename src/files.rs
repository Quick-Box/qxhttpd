use sqlx::{FromRow};
use rocket::{Build, Data, Rocket, State};
use rocket::data::ToByteUnit;
use rocket::http::{ContentType, Status};
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use crate::db::DbPool;
use crate::event::{load_event_info2, EventId};
use crate::{unzip_data, QxApiToken};

#[derive(Serialize, Deserialize, FromRow)]
pub struct FileInfo {
    pub name: String,
    pub size: i64,
    pub created: chrono::DateTime<chrono::Utc>,
}
pub async fn list_files(event_id: EventId, db: &State<DbPool>) -> Result<Vec<FileInfo>, String> {
    println!("listing files of event: {event_id}");
    let pool = &db.0;
    let files = sqlx::query_as::<_, FileInfo>("SELECT id, name, LENGTH(data) AS size, created FROM files WHERE event_id=?")
        .bind(event_id)
        .fetch_all(pool).await.map_err(|e| {
            // println!("listing files error: {:?}", e);
            e.to_string()
    });
    files
}
#[get("/api/event/<event_id>/file")]
async fn get_files(event_id: EventId, db: &State<DbPool>) -> Result<Json<Vec<FileInfo>>, Custom<String>> {
    let files = list_files(event_id, db).await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(files))
}
#[get("/api/event/<event_id>/file/<file_name>")]
async fn get_file(event_id: EventId, file_name: &str, db: &State<DbPool>) -> Result<Vec<u8>, String> {
    let files = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND name=?")
        .bind(event_id)
        .bind(file_name)
        .fetch_one(&db.0).await;
    files.map(|d| d.0 ).map_err(|e| e.to_string())
}
#[post("/api/event/current/file/<file_name>", data = "<data>")]
async fn post_file(qx_api_token: QxApiToken, file_name: &str, data: Data<'_>, content_type: &ContentType, db: &State<DbPool>) -> Result<(), Custom<String>> {
    let event_info = load_event_info2(&qx_api_token, db).await?;
    let data = data.open(50.mebibytes()).into_bytes().await.map_err(|e| Custom(Status::BadRequest, e.to_string()))?.into_inner();
    let result = if data.is_empty() {
        info!("Event id: {}, deleting file: {file_name}", event_info.id);
        sqlx::query("DELETE FROM files WHERE event_id=? AND name=?;")
            .bind(event_info.id)
            .bind(file_name)
            .execute(&db.0).await
    } else {
        let q = sqlx::query("INSERT OR REPLACE INTO files (event_id, name, data) VALUES (?, ?, ?);")
            .bind(event_info.id)
            .bind(file_name);
        let q = if content_type == &ContentType::ZIP {
            let decompressed = unzip_data(&data).map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
            info!("Event id: {}, updating file: {file_name} with {} bytes of data", event_info.id, decompressed.len());
            q.bind(decompressed)
        } else {
            info!("Event id: {}, updating file: {file_name} with {} bytes of data", event_info.id, data.len());
            q.bind(data)
        };
        q.execute(&db.0).await
    };
    result.map(|_n| ()).map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            get_files,
            get_file,
            post_file,
        ])
}