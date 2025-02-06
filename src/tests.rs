use rocket::local::blocking::Client;
use rocket::http::{ContentType, Header, Status};
use crate::{QERunsRecord};

#[test]
fn api_register_event() {
    let client = Client::tracked(super::rocket()).unwrap();
    let rq = client.post("/api/event/")
        .header(Header::new("x-register-token", "28f40c17d8c2edba71b1f59d67de6964719c4e11"));
    let resp = rq.dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.content_type(), Some(ContentType::JSON));
    let records = resp.into_json::<Vec<QERunsRecord>>().unwrap();
    assert_eq!(records.len(), 1);
}

#[test]
fn api_qe_chng_in() {
    let client = Client::tracked(super::rocket()).unwrap();
    let response = client.get("/api/event/1/qe/chng/in?offset=0&limit=1").dispatch();
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
    let records = response.into_json::<Vec<QERunsRecord>>().unwrap();
    assert_eq!(records.len(), 1);
}

