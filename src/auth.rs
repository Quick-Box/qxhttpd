use anyhow::{anyhow, Context};
use rand::Rng;
use reqwest::header::AUTHORIZATION;
use rocket::{get, routes, Build, Rocket, State};
use rocket::http::{Cookie, CookieJar, SameSite};
use rocket::log::private::info;
use rocket::response::{Debug, Redirect};
use rocket_oauth2::{OAuth2, TokenResponse};
use serde_json::Value;
use crate::{QxSession, QxSessionId, SharedQxState};

#[derive(Clone, serde::Serialize)]
pub struct UserInfo {
    name: String,
    pub(crate) email: String,
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

pub fn generate_random_string(len: usize) -> String {
    const WOWELS: &str = "aeiouy";
    const CONSONANTS: &str = "bcdfghjklmnopqrstvwxz";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|n| {
            let charset = if n % 2 == 0 { CONSONANTS } else { WOWELS };
            let idx = rng.gen_range(0..charset.len());
            charset.chars().nth(idx).unwrap()
        })
        .collect()
}
pub const QX_SESSION_ID: &str = "qx_session_id";

/// User information to be retrieved from the Google People API.
#[derive(serde::Deserialize)]
#[allow(non_snake_case)]
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
fn google_login(oauth2: OAuth2<GoogleUserInfo>, cookies: &CookieJar<'_>) -> Redirect {
    oauth2.get_redirect(cookies, &["profile", "email"]).unwrap()
}

#[get("/auth/google")]
async fn google_auth(token: TokenResponse<GoogleUserInfo>, cookies: &CookieJar<'_>, state: &State<SharedQxState>) -> Result<Redirect, Debug<anyhow::Error>> {
    // Use the token to retrieve the user's Google account information.
    info!("=====> google_callback ==============");
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
    let google_user_info: GoogleUserInfo = response
        .json()
        .await
        .context("failed to deserialize response")?;
    let user_info = UserInfo::try_from(&google_user_info)?;
    fn generate_session_id() -> String {
        generate_random_string(32)
    }
    let session_id = generate_session_id();
    info!("name: {}, email: {}", user_info.name, user_info.email);
    state.write().expect("not poisoned").sessions.insert(QxSessionId(session_id.clone()), QxSession{ user_info });
    // Set a private cookie with the user's name, and redirect to the home page.
    cookies.add_private(
        Cookie::build((QX_SESSION_ID, session_id))
            .same_site(SameSite::Lax)
            .build()
    );
    Ok(Redirect::to("/"))
}

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
            login,
            google_login,
            google_auth,
        ])
        .attach(OAuth2::<GoogleUserInfo>::fairing("google"))
}