#[macro_use] extern crate rocket;

use sqlx::Sqlite;
use sqlx::Encode;
use std::sync::Arc;
use crate::event::{user_info_opt, EventId, EventRecord};
use std::fmt::{Debug, Display, Formatter};
use std::collections::{HashMap};
use std::sync::atomic::AtomicU64;
use rocket::fs::{FileServer};
use rocket::{request, tokio, State};
use rocket::http::{CookieJar, Status};
use rocket::response::{status};
use rocket::response::status::{Custom};
use rocket_dyn_templates::{Template, context, handlebars};
use rocket::serde::Serialize;
use rocket_dyn_templates::handlebars::{Handlebars, Helper};
use serde::{Deserialize};
use sqlx::SqlitePool;
use crate::auth::{UserInfo, QX_SESSION_ID};
use crate::changes::{ChangesRecord, QxRunChange};
use crate::db::{DbPool, DbPoolFairing};
use crate::qxdatetime::{dtstr, obtime, obtimems};
use crate::util::anyhow_to_custom_error;
use async_broadcast::{broadcast};
use sqlx::sqlite::{SqliteArgumentValue};

#[cfg(test)]
mod tests;
mod db;
mod auth;
mod oc;
mod event;
mod files;
mod util;
mod iofxml3;
mod qxdatetime;
mod runs;
mod changes;

struct AppConfig {
    server_address: String,
    server_port: u16,
    db_path: String,
}
impl AppConfig {
    pub fn is_local_server(&self) -> bool {
        self.server_address == "127.0.0.1"
    }
}
struct QxSession {
    user_info: UserInfo,
}
#[derive(Eq, Hash, PartialEq)]
struct QxSessionId(String);
#[rocket::async_trait]
impl<'r> request::FromRequest<'r> for QxSessionId {
    type Error = ();
    async fn from_request(request: &'r request::Request<'_>) -> request::Outcome<Self, ()> {
        let cookies = request
            .guard::<&CookieJar<'_>>()
            .await
            .expect("request cookies");
        if let Some(cookie) = cookies.get_private(QX_SESSION_ID) {
            return request::Outcome::Success(Self(cookie.value().to_string()));
        }
        request::Outcome::Forward(Status::Unauthorized)
    }
}

enum MaybeSessionId {
    None,
    Some(QxSessionId),
}
#[rocket::async_trait]
impl<'r> request::FromRequest<'r> for MaybeSessionId {
    type Error = ();
    async fn from_request(request: &'r request::Request<'_>) -> request::Outcome<Self, ()> {
        let cookies = request
            .guard::<&CookieJar<'_>>()
            .await
            .expect("request cookies");
        if let Some(cookie) = cookies.get_private(QX_SESSION_ID) {
            return request::Outcome::Success(Self::Some(QxSessionId(cookie.value().to_string())));
        }
        request::Outcome::Success(Self::None)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Default, Clone, Debug)]
struct QxApiToken(String);

impl QxApiToken {
    pub fn from_str(s: &str) -> Self {
        QxApiToken(s.to_owned())
    }
}

impl_sqlx_text_type_encode_decode!(QxApiToken);

impl Display for QxApiToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
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

struct OpenEvent {
    hit_count: Arc<AtomicU64>,
    db: SqlitePool,
}
struct QxState {
    app_config: AppConfig,
    sessions: HashMap<QxSessionId, QxSession>,
    open_events: HashMap<EventId, OpenEvent>,
    changes_sender: async_broadcast::Sender<(EventId, ChangesRecord)>,
    changes_receiver: async_broadcast::Receiver<(EventId, ChangesRecord)>,
    runs_changes_sender: async_broadcast::Sender<(EventId, QxRunChange)>,
    runs_changes_receiver: async_broadcast::Receiver<(EventId, QxRunChange)>,
}
impl QxState {
    fn new(app_config: AppConfig) -> Self {
        let (mut changes_sender, changes_receiver) = broadcast(2);
        changes_sender.set_overflow(true);
        let (mut runs_changes_sender, runs_changes_receiver) = broadcast(2);
        runs_changes_sender.set_overflow(true);
        Self {
            app_config,
            sessions: Default::default(),
            open_events: Default::default(),
            changes_sender,
            changes_receiver,
            runs_changes_sender,
            runs_changes_receiver,
        }
    }
    async fn broadcast_change(&self, chng: (EventId, ChangesRecord)) -> anyhow::Result<()> {
        self.changes_sender.broadcast(chng).await?;
        Ok(())
    }
    async fn broadcast_runs_change(&self, chng: (EventId, QxRunChange)) -> anyhow::Result<()> {
        self.runs_changes_sender.broadcast(chng).await?;
        Ok(())
    }
}
type SharedQxState = tokio::sync::RwLock<QxState>;

#[get("/")]
async fn index(sid: MaybeSessionId, state: &State<SharedQxState>, db: &State<DbPool>) -> Result<Template, Custom<String>> {
    let user = user_info_opt(sid, state).await.map_err(anyhow_to_custom_error)?;
    let pool = &db.0;
    let events: Vec<EventRecord> = sqlx::query_as("SELECT * FROM events")
        .fetch_all(pool)
        .await
        .map_err(|e| status::Custom(Status::InternalServerError, e.to_string()))?;
    let is_local_server = state.read().await.app_config.is_local_server();
    Ok(Template::render("index", context! {
        user,
        events,
        show_create_demo: is_local_server,
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
                                           let s = dtstr(val.as_str());
                                           out.write(&s)?;
                                           Ok(())
                                       }));
            handlebars.register_helper("obtime",
                                       Box::new(|h: &Helper, _r: &Handlebars, _: &handlebars::Context, _rc: &mut handlebars::RenderContext, out: &mut dyn handlebars::Output| -> handlebars::HelperResult {
                                           let val = h.param(0).ok_or(handlebars::RenderErrorReason::ParamNotFoundForIndex("obtime", 0))?.value();
                                           if let Some(sec) = val.as_i64() {
                                              let s = obtime(sec);
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
                                               let s = obtimems(msec);
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
            index,
        ]);
    let rocket = auth::extend(rocket);
    let rocket = event::extend(rocket);
    let rocket = oc::extend(rocket);
    let rocket = runs::extend(rocket);
    let rocket = changes::extend(rocket);
    let rocket = files::extend(rocket);

    let figment = rocket.figment();
    let server_address = figment.extract_inner::<String>("address").expect("server address");
    let server_port = figment.extract_inner::<u16>("port").expect("Server port");
    let db_path = figment.extract_inner::<String>("db_path").expect("db_path");
    
    let cfg = AppConfig{ server_address, server_port, db_path };

    let state = QxState::new(cfg);
    rocket.manage(SharedQxState::new(state))
}

