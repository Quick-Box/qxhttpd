use rocket::http::{Cookie, CookieJar, SameSite};
use rocket::response::{Debug, Redirect};
use rocket_oauth2::{OAuth2, TokenResponse};
use serde_json::Value;
use anyhow::{anyhow, Context, Error};
use reqwest::header::AUTHORIZATION;
use rocket::{Build, Rocket, State};
use crate::{api, QxSession, QxSessionId, SharedQxState};

#[derive(Clone, serde::Serialize)]
pub struct UserInfo {
    name: String,
    email: String,
    picture: String,
}
impl TryFrom<&GoogleUserInfo> for UserInfo {
    type Error = anyhow::Error;

    fn try_from(info: &GoogleUserInfo) -> Result<Self, Self::Error> {
        let picture = info.picture.to_string();
        let email = info.email.to_string();
        if email.is_empty() {
            return Err(anyhow!("User email must be set"));
        };
        
        Ok(Self {
            name: info.name.to_string(),
            email,
            picture,
        })
    }
}
#[derive(serde::Deserialize)]
struct GoogleUserInfo {
    name: Value,
    email: Value,
    picture: Value,
}

#[get("/login")]
fn login() -> Redirect {
    Redirect::to("/login/google")
}

#[get("/login/google")]
fn login_google(oauth2: OAuth2<GoogleUserInfo>, cookies: &CookieJar<'_>) -> Redirect {
    oauth2.get_redirect(cookies, &["profile", "email"]).unwrap()
}
pub const QX_SESSION_ID: &str = "qx_session_id";
#[get("/auth/google")]
async fn auth_google_callback(token: TokenResponse<GoogleUserInfo>, cookies: &CookieJar<'_>, state: &State<SharedQxState>) -> Result<Redirect, Debug<Error>> {
    // Use the token to retrieve the user's Google account information.
    info!("=====> auth_google_callback ==============");
    let rq = reqwest::Client::builder()
        .build()
        .context("failed to build reqwest client")?
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        // .get("https://people.googleapis.com/v1/people/me?personFields=names,emailAddresses")
        .header(AUTHORIZATION, format!("Bearer {}", token.access_token()));
    info!("=====> user name: {:?}", rq);
    let response = rq.send()
        .await
        .context("failed to complete request")?;
    // info!("=====> response ==============: {:?}", response.text().await);
    let googole_user_info: GoogleUserInfo = response
        .json()
        .await
        .context("failed to deserialize response")?;
    let user_info = UserInfo::try_from(&googole_user_info)?;
    fn generate_session_id() -> String {
        api::generate_random_string(32)
    }
    let session_id = generate_session_id();
    info!("session: {session_id}, email: {}", user_info.email);
    state.write().expect("not poisoned").sessions.insert(QxSessionId(session_id.clone()), QxSession { user_info });
    // Set a private cookie with the user's name, and redirect to the home page.
    cookies.add_private(
        Cookie::build((QX_SESSION_ID, session_id))
            .same_site(SameSite::Lax)
            .build()
    );
    Ok(Redirect::to("/"))
}

pub fn mount(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            login,
            login_google,
            auth_google_callback,
        ])
        .attach(OAuth2::<GoogleUserInfo>::fairing("google"))
}