use rocket::{Build, Rocket};

pub fn extend(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/", routes![
    ])
}

