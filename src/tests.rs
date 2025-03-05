use std::str::FromStr;
use chrono::NaiveDateTime;
use rocket::local::blocking::Client;
use rocket::http::{ContentType, Header, Status};
use crate::event::{EventInfo, PostedEvent};

#[test]
fn create_demo_event() {
    let client = Client::tracked(super::rocket()).unwrap();
    let resp = client.get("/event/create-demo").dispatch();
    // println!("body: {:?}", resp.body());
    assert_eq!(resp.status(), Status::SeeOther);
    
    let resp = client.get("/api/event/current")
        .header(Header::new("qx-api-token", "plelababamak"))
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
        .header(Header::new("qx-api-token", "plelababamak"))
        .json(&post_event)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    let resp = client.get("/api/event/current")
        .header(Header::new("qx-api-token", "plelababamak"))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.content_type(), Some(ContentType::JSON));
    let event = resp.into_json::<EventInfo>().unwrap();
    assert_eq!(event.name, post_event.name);
    assert_eq!(event.place, post_event.place);
    assert_eq!(event.start_time, post_event.start_time);
}

// #[test]
// fn api_qe_chng_in() {
//     let client = Client::tracked(super::rocket()).unwrap();
//     let response = client.get("/api/event/1/qe/chng/in?offset=0&limit=1").dispatch();
//     assert_eq!(response.status(), Status::Ok);
//     assert_eq!(response.content_type(), Some(ContentType::JSON));
//     let records = response.into_json::<Vec<QERunsRecord>>().unwrap();
//     assert_eq!(records.len(), 1);
// }

