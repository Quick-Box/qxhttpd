use sqlx::{FromRow};
use rocket::{Build, Data, Rocket, State};
use rocket::data::ToByteUnit;
use serde::{Deserialize, Serialize};
use crate::db::DbPool;
use crate::event::EventId;

// #[derive(FromRow)]
// struct FileRecord {
//     file_name: String,
//     data: Vec<u8>,
//     created: chrono::DateTime<chrono::Utc>,
// }
#[derive(Serialize, Deserialize, FromRow)]
pub struct FileInfo {
    pub name: String,
    pub size: i64,
    pub created: chrono::DateTime<chrono::Utc>,
}
pub async fn list_files(event_id: EventId, db: &State<DbPool>) -> Vec<FileInfo> {
    let pool = &db.0;
    let files = sqlx::query_as::<_, FileInfo>("SELECT id, file_name, LENGTH(data) AS file_size, created FROM files WHERE event_id=?")
        .bind(event_id)
        .fetch_all(pool).await;
    //.await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    files.unwrap_or(vec![])
}

#[get("/event/<event_id>/files/<file_name>")]
async fn get_file(event_id: EventId, file_name: &str, db: &State<DbPool>) -> Result<Vec<u8>, String> {
    let files = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM files WHERE event_id=? AND file_name=?")
        .bind(event_id)
        .bind(file_name)
        .fetch_one(&db.0).await;
    files.map(|d| d.0 ).map_err(|e| e.to_string())
}
#[post("/event/<event_id>/files/<file_name>", data = "<data>")]
async fn set_file(event_id: EventId, file_name: &str, data: Data<'_>, db: &State<DbPool>) -> Result<(), String> {
    let data = data.open(50.mebibytes()).into_bytes().await.map_err(|e| e.to_string())?.into_inner();
    let result = if data.is_empty() {
        info!("Event: {event_id}, deleting file: {file_name}");
        sqlx::query("DELETE FROM files WHERE event_id=? AND file_name=?;")
            .bind(event_id)
            .bind(file_name)
            .execute(&db.0).await
    } else {
        info!("Event: {event_id}, updating file: {file_name} with {} bytes of data", data.len());
        sqlx::query("INSERT OR REPLACE INTO files (event_id, file_name, data) VALUES (?, ?, ?);")
            .bind(event_id)
            .bind(file_name)
            .bind(data)
            .execute(&db.0).await
    };
    result.map(|_n| ()).map_err(|e| e.to_string())
}
pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            //get_files_list,
            get_file,
            set_file,
        ])
}