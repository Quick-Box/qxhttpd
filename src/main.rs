#[macro_use] extern crate rocket;

use crate::event::{EventInfo};
use std::fmt::Debug;
use std::collections::{HashMap};
use std::sync::RwLock;
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
mod oc;
mod event;
mod files;
mod util;
mod iofxml3;
mod qe;

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

#[get("/")]
async fn index_anonymous(db: &State<DbPool>) -> std::result::Result<Template, Custom<String>> {
    let pool = &db.0;
    let events: Vec<EventInfo> = sqlx::query_as("SELECT * FROM events")
        .fetch_all(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Template::render("index", context! {
        events,
    }))
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
                                           let s = util::dtstr(val.as_str());
                                           out.write(&s)?;
                                           Ok(())
                                       }));
            handlebars.register_helper("obtime",
                                       Box::new(|h: &Helper, _r: &Handlebars, _: &handlebars::Context, _rc: &mut handlebars::RenderContext, out: &mut dyn handlebars::Output| -> handlebars::HelperResult {
                                           let val = h.param(0).ok_or(handlebars::RenderErrorReason::ParamNotFoundForIndex("obtime", 0))?.value();
                                           if let Some(sec) = val.as_i64() {
                                              let s = util::obtime(sec);
                                              out.write(&s)?;
                                           } else {
                                              out.write("--:--:--")?;
                                           }
                                           Ok(())
                                       }));
            handlebars.register_helper("obtimems",
                                       Box::new(|h: &Helper, _r: &Handlebars, _: &handlebars::Context, _rc: &mut handlebars::RenderContext, out: &mut dyn handlebars::Output| -> handlebars::HelperResult {
                                           let val = h.param(0).ok_or(handlebars::RenderErrorReason::ParamNotFoundForIndex("obtime", 0))?.value();
                                           if let Some(msec) = val.as_i64() {
                                               let s = util::obtimems(msec);
                                               out.write(&s)?;
                                           } else {
                                               out.write("--:--:--")?;
                                           }
                                           Ok(())
                                       }));
        }))
        .attach(DbPoolFairing())
        .mount("/", FileServer::from("./static"))
        .mount("/", routes![
            index_anonymous,
        ]);
    let rocket = auth::extend(rocket);
    let rocket = event::extend(rocket);
    let rocket = oc::extend(rocket);
    let rocket = qe::extend(rocket);
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

