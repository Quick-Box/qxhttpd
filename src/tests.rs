use std::str::FromStr;
use chrono::NaiveDateTime;
use rocket::local::blocking::Client;
use rocket::http::{ContentType, Header, Status};
use crate::event::{EventId, EventInfo, PostedEvent};
use crate::files::FileInfo;
use crate::zip_data;

const API_TOKEN: &str = "plelababamak";

fn create_test_server() -> Client {
    let client = Client::tracked(super::rocket()).unwrap();
    {
        let resp = client.get("/event/create-demo").dispatch();
        // println!("body: {:?}", resp.body());
        assert_eq!(resp.status(), Status::SeeOther);
    }
    client
}

#[test]
fn update_event_data() {
    let client = create_test_server();
    
    let resp = client.get("/api/event/current")
        .header(Header::new("qx-api-token", API_TOKEN))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.content_type(), Some(ContentType::JSON));
    let event = resp.into_json::<EventInfo>().unwrap();
    assert_eq!(event.id, 1);
    
    let dt = NaiveDateTime::from_str("2025-03-04T10:20:00").unwrap();
    let post_event = PostedEvent {
        name: "Foo".to_string(),
        place: "Bar".to_string(),
        start_time: dt,
    };
    let resp = client.post("/api/event/current")
        .header(Header::new("qx-api-token", API_TOKEN))
        .json(&post_event)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    let resp = client.get("/api/event/current")
        .header(Header::new("qx-api-token", API_TOKEN))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.content_type(), Some(ContentType::JSON));
    let event = resp.into_json::<EventInfo>().unwrap();
    assert_eq!(event.name, post_event.name);
    assert_eq!(event.place, post_event.place);
    assert_eq!(event.start_time, post_event.start_time);
}

#[test]
fn upload_file() {
    let client = create_test_server();
    const EVENT_ID: EventId = 1;

    // send file
    let file_name = "a.txt";
    let orig = b"foo-bar-baz";
    let compressed = zip_data(orig).unwrap();
    let resp = client.post(format!("/api/event/current/file/{file_name}"))
        .header(Header::new("qx-api-token", API_TOKEN))
        .header(ContentType::ZIP)
        .body(compressed)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // get this file
    let resp = client.get(format!("/api/event/{EVENT_ID}/file/{file_name}"))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let content = resp.into_bytes().unwrap();
    assert_eq!(&content, orig);

    // list files
    let resp = client.get(format!("/api/event/{EVENT_ID}/file")).dispatch();
    // println!("bodycko: {:?}", resp.into_string());
    assert_eq!(resp.status(), Status::Ok);
    let files = resp.into_json::<Vec<FileInfo>>().unwrap();
    assert_eq!(files.iter().len(), 1);
    assert_eq!(&files.first().unwrap().name, file_name);
}


