#[macro_use] extern crate rocket;

use crate::event::{user_info, EventInfo};
use std::fmt::Debug;
use std::collections::{HashMap};
use std::io::Read;
use std::sync::RwLock;
use chrono::NaiveDateTime;
use flate2::bufread::{ZlibEncoder};
use flate2::Compression;
use rocket::fs::{FileServer};
use rocket::{request, State};
use rocket::http::{CookieJar, Status};
use rocket::response::{status};
use rocket::response::status::{Custom};
use rocket_dyn_templates::{Template, context, handlebars};
use rocket::serde::Serialize;
use rocket_dyn_templates::handlebars::{Handlebars, Helper};
use serde::{Deserialize};
use crate::auth::{UserInfo, QX_SESSION_ID};
use crate::db::{DbPool, DbPoolFairing};

#[cfg(test)]
mod tests;
mod db;
mod auth;
mod ochecklist;
mod quickevent;
mod event;
mod files;

#[derive(Default)]
struct AppConfig {
}
struct QxSession {
    user_info: UserInfo,
}
#[derive(Eq, Hash, PartialEq)]
struct QxSessionId(String);
#[rocket::async_trait]
impl<'r> request::FromRequest<'r> for QxSessionId {
    type Error = ();
    async fn from_request(request: &'r request::Request<'_>) -> request::Outcome<QxSessionId, ()> {
        let cookies = request
            .guard::<&CookieJar<'_>>()
            .await
            .expect("request cookies");
        if let Some(cookie) = cookies.get_private(QX_SESSION_ID) {
            return request::Outcome::Success(QxSessionId(cookie.value().to_string()));
        }
        request::Outcome::Forward(Status::Unauthorized)
    }
}
#[derive(Serialize, Deserialize, PartialEq, Default, Clone, Debug)]
struct QxApiToken(String);
impl_sqlx_text_type_and_decode!(QxApiToken);

#[rocket::async_trait]
impl<'r> request::FromRequest<'r> for QxApiToken {
    type Error = ();
    async fn from_request(request: &'r request::Request<'_>) -> request::Outcome<QxApiToken, ()> {
        if let Some(api_token) = request.headers().get_one("qx-api-token") {
            return request::Outcome::Success(QxApiToken(api_token.to_string()));
        }
        request::Outcome::Forward(Status::Unauthorized)
    }
}
struct QxState {
    sessions: HashMap<QxSessionId, QxSession>,
}
impl QxState {
}
type SharedQxState = RwLock<QxState>;

async fn index(user: Option<UserInfo>, db: &State<DbPool>) -> std::result::Result<Template, status::Custom<String>> {
    let pool = &db.0;
    let events: Vec<EventInfo> = sqlx::query_as("SELECT * FROM events")
        .fetch_all(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Template::render("index", context! {
            user,
            events,
        }))
}

#[get("/")]
async fn index_authorized(session_id: QxSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info(session_id, state).map_err(|e| Custom(Status::Unauthorized, e))?;
    index(Some(user), db).await
}
#[get("/", rank = 2)]
async fn index_anonymous(db: &State<DbPool>) -> std::result::Result<Template, Custom<String>> {
    index(None, db).await
}
#[launch]
fn rocket() -> _ {
    let rocket = rocket::build()
        // .attach(Template::fairing())
        .attach(Template::custom(|engines| {
            let handlebars = &mut engines.handlebars;

            // Register a custom Handlebars helper
            handlebars.register_helper("stringify",
                                       Box::new(|h: &Helper, _r: &Handlebars, _: &handlebars::Context, _rc: &mut handlebars::RenderContext, out: &mut dyn handlebars::Output| -> handlebars::HelperResult {
                                           let param = h.param(0).ok_or(handlebars::RenderErrorReason::ParamNotFoundForIndex("stringify", 0))?;
                                           let json = serde_json::to_string(param.value()).unwrap_or_else(|_| "Invalid JSON".to_string());
                                           // out.write("3rd helper: ")?;
                                           // out.write(param.value().render().as_ref())?;
                                           out.write(json.as_ref())?;
                                           Ok(())
                                       }));
            handlebars.register_helper("dtstr",
                                       Box::new(|h: &Helper, _r: &Handlebars, _: &handlebars::Context, _rc: &mut handlebars::RenderContext, out: &mut dyn handlebars::Output| -> handlebars::HelperResult {
                                           let val = h.param(0).ok_or(handlebars::RenderErrorReason::ParamNotFoundForIndex("dtstr", 0))?.value();
                                           let s = dtstr(val.as_str());
                                           out.write(&s)?;
                                           Ok(())
                                       }));
        }))
        .attach(DbPoolFairing())
        .mount("/", FileServer::from("./static"))
        .mount("/", routes![
            index_authorized,
            index_anonymous,
        ]);
    let rocket = auth::extend(rocket);
    let rocket = event::extend(rocket);
    let rocket = ochecklist::extend(rocket);
    let rocket = quickevent::extend(rocket);
    let rocket = files::extend(rocket);

    let cfg = AppConfig::default();
    // let figment = rocket.figment();
    // let create_demo_event = figment.extract_inner::<bool>("qx_create_demo_event").ok().unwrap_or(false);

    let rocket = rocket.manage(cfg);

    let state = QxState {
        sessions: Default::default(),
    };

    rocket.manage(SharedQxState::new(state))
}

fn dtstr(iso_date_str: Option<&str>) -> String {
    let Some(s) = iso_date_str else {
        return "--/--/--".to_string()
    };
    let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") else {
        return s.to_string()
    };
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn parse_naive_datetime(datetime_str: &str) -> Option<NaiveDateTime> {
    for &format in &[
        "%Y-%m-%d %H:%M:%S",       // 2025-03-05 14:32:45
        "%Y-%m-%d %H:%M",          // 2025-03-05 14:32
        "%Y-%m-%dT%H:%M:%S",       // 2025-03-05T14:32:45
        "%Y-%m-%dT%H:%M",          // 2025-03-05T14:32
        "%d/%m/%Y %H:%M:%S",       // 05/03/2025 14:32:45
        // "%m/%d/%Y %H:%M:%S",       // 03/05/2025 14:32:45
        // "%Y/%m/%d %H:%M:%S",       // 2025/03/05 14:32:45
        // "%Y-%m-%d",                // 2025-03-05
        // "%m/%d/%Y",                // 03/05/2025
        "%d/%m/%Y",                // 05/03/2025
        "%H:%M:%S",                // 14:32:45
    ] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(datetime_str, format) {
            return Some(parsed);
        }
    }

    // Return None if no format matched
    None
}

fn unzip_data(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut z = flate2::read::ZlibDecoder::new(bytes);
    let mut s = Vec::new();
    z.read_to_end(&mut s).map_err(|e| { e.to_string() })?;
    Ok(s)
}

fn zip_data(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut ret_vec = Vec::new();
    let mut deflater = ZlibEncoder::new(bytes, Compression::fast());
    deflater.read_to_end(&mut ret_vec).map_err(|e| e.to_string())?;
    Ok(ret_vec)
}

#[cfg(test)]
mod test_main {
    use crate::{unzip_data, zip_data};

    #[test]
    fn test_zip() {
        let data = b"foo bar baz";
        let zdata = zip_data(data).unwrap();
        let udata = unzip_data(&zdata).unwrap();
        assert_eq!(udata, data);
    }
}

