use rocket::local::blocking::Client;
use rocket::http::{ContentType, Status};
use crate::QEChangeRecord;

#[test]
fn api_qe_chng_in() {
    let client = Client::tracked(super::rocket()).unwrap();
    let response = client.get("/api/event/0/qe/chng/in?offset=0&limit=1").dispatch();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    let records = response.into_json::<Vec<QEChangeRecord>>().unwrap();
    assert_eq!(records.len(), 1);
}

