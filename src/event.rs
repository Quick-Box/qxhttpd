use rocket::http::Status;
use rocket::response::status;
use rocket::response::status::Custom;
use rocket::State;
use crate::db::DbPool;
use crate::{EventId, EventInfo};

pub async fn load_event_info(event_id: EventId, db: &State<DbPool>) -> Result<EventInfo, Custom<String>> {
    let pool = &db.0;
    let event: EventInfo = sqlx::query_as("SELECT * FROM events WHERE id=?")
        .bind(event_id)
        .fetch_one(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    Ok(event)
}