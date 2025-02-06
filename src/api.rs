use rocket::http::Status;
use rocket::{Build, Data, Request, Rocket, State};
use rocket::data::ToByteUnit;
use rocket::request::{FromRequest, Outcome};
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::Json;
use crate::{EventId, EventInfo, OCheckListChangeSet, QERunsRecord, QxState};

pub fn mount(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/api/", routes![
            api_get_qe_in_changes,
            api_add_oc_change_set
        ])
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct ApiKey(String);

impl ApiKey {
    pub(crate) fn generate() -> Self {
        Self("42".to_string())
    }
}

#[derive(Debug)]
enum ApiKeyError {
    Missing,
    Invalid,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
struct RegisterEventResponse {
    eventId: EventId,
    apiKey: ApiKey,
}

// struct RegisterToken<'r>(&'r str);
#[rocket::async_trait]
impl<'r> FromRequest<'r> for ApiKey {
    type Error = ApiKeyError;
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        fn is_valid(_key: &str) -> bool {
            true
        }
        match req.headers().get_one("x-api-key") {
            None => Outcome::Error((Status::BadRequest, ApiKeyError::Missing)),
            Some(key) if is_valid(key) => Outcome::Success(ApiKey(key.to_string())),
            Some(_) => Outcome::Error((Status::BadRequest, ApiKeyError::Invalid)),
        }
    }
}
#[post("/event", data = "<data>")]
async fn api_register_event(api_key: ApiKey, data: Data<'_>, state: &State<crate::SharedQxState>) -> std::result::Result<Json<RegisterEventResponse>, String> {
    let mut buffer = String::new();
    let content = data.open(128.kibibytes()).into_string().await.map_err(|err| err.to_string())?;
    let event_info: EventInfo = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let mut qx_state = state.write().unwrap();
    let event_id = QxState::create_event(event_info).map_err(|e| e.to_string())?;
    let api_key = state.read().unwrap().events.get(&event_id).expect("key must exist").read().unwrap().event.api_key.clone();
    Ok(Json(RegisterEventResponse { eventId: event_id, apiKey: api_key }))
}
#[get("/event/<event_id>/qe/chng/in?<offset>&<limit>")]
fn api_get_qe_in_changes(event_id: EventId, offset: Option<i32>, limit: Option<i32>, state: &State<crate::SharedQxState>) -> Json<Vec<QERunsRecord>> {
    let state = state.read().unwrap();
    let event = state.events.get(&event_id).unwrap().read().unwrap();
    let offset = offset.unwrap_or(0) as usize;
    let lst = event.qe.get_records(offset, limit.map(|l| l as usize)).unwrap();
    Json(lst)
}
#[post("/event/<event_id>/oc", data = "<data>")]
async fn api_add_oc_change_set(event_id: EventId, data: Data<'_>, state: &State<crate::SharedQxState>) -> std::result::Result<(), String> {
    let content = data.open(128.kibibytes()).into_string().await.map_err(|err| err.to_string())?;
    let oc: OCheckListChangeSet = serde_yaml::from_str(&content).unwrap();
    state.write().unwrap().add_oc_change_set(event_id, oc).map_err(|err| err.to_string())?;
    Ok(())
}
