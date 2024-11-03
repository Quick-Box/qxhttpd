#[macro_use] extern crate rocket;

use std::io;
use std::path::{PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use rocket::data::{Data, ToByteUnit};
use rocket::response::content::RawText;
use rocket::tokio::fs::{self, File};
use rocket_dyn_templates::{Template, context};

// In a real application, these would be retrieved dynamically from a config.
// const HOST: Absolute<'static> = uri!("http://*:8000");
const DATA_DIR: &'static str = "/tmp/qxhttpd/data";
static RECENT_RECORD_ID: AtomicI32 = AtomicI32::new(0);

type RecordId = i32;
fn next_record_id() -> RecordId {
    let old_id = RECENT_RECORD_ID.fetch_add(1, Ordering::SeqCst);
    old_id + 1
}
fn record_file(id: RecordId) -> io::Result<PathBuf> {
    std::fs::create_dir_all(DATA_DIR)?;
    Ok(PathBuf::from(DATA_DIR).join(format!("{:04}", id)))
}

#[post("/", data = "<paste>")]
async fn upload(paste: Data<'_>) -> io::Result<String> {
    let id = next_record_id();
    let file_path = record_file(id)?;
    println!("file path: {:?}", file_path);
    paste.open(128.kibibytes()).into_file(file_path).await?;
    Ok(id.to_string())
}

#[get("/api/<id>")]
async fn retrieve(id: RecordId) -> Option<RawText<File>> {
    let Ok(file_path) = record_file(id) else { return None };
    File::open(file_path).await.map(RawText).ok()
}

#[delete("/<id>")]
async fn delete(id: RecordId) -> Option<()> {
    let Ok(file_path) = record_file(id) else { return None };
    fs::remove_file(file_path).await.ok()
}

#[get("/")]
fn index() -> Template {
    Template::render("index", context! {
        title: "Hello",
        name: Some("Fanda"),
        items: vec!["One", "Two", "Three"],
    })
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(Template::fairing())
        .mount("/", routes![index, upload, delete, retrieve])
}
