use crate::event::START_LIST_IOFXML3_FILE;
use std::fs::OpenOptions;
use std::io::{Read};
use rocket::local::blocking::Client;
use rocket::http::{ContentType, Header, Status};
use crate::event::{EventId, Eventrecord, PostedEvent};
use crate::files::FileInfo;
use crate::qe::QERunChange;
use crate::qe::runs::RunsRecord;
use crate::qxdatetime::QxDateTime;
use crate::util;

const API_TOKEN: &str = "plelababamak";
const EVENT_ID: EventId = 1;

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
    let event = resp.into_json::<Eventrecord>().unwrap();
    assert_eq!(event.id, 1);

    let dt = QxDateTime::now().trimmed_to_sec();
    let post_event = PostedEvent {
        name: "Foo".to_string(),
        place: "Bar".to_string(),
        start_time: dt.0,
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
    let event = resp.into_json::<Eventrecord>().unwrap();
    assert_eq!(event.name, post_event.name);
    assert_eq!(event.place, post_event.place);
    assert_eq!(event.start_time.0, post_event.start_time);
}

#[test]
fn upload_file() {
    let client = create_test_server();

    // send file
    let file_name = "a.txt";
    let orig = b"foo-bar-baz";
    let compressed = util::test::zip_data(orig).unwrap();
    let resp = client.post(format!("/api/event/current/file?name={file_name}"))
        .header(Header::new("qx-api-token", API_TOKEN))
        .header(ContentType::ZIP)
        .body(compressed)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let file_id = resp.into_string().unwrap().parse::<i32>().unwrap();
    assert_eq!(file_id, 1);

    // get this file
    let resp = client.get(format!("/api/event/{EVENT_ID}/file/{file_id}")).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let content = resp.into_bytes().unwrap();
    assert_eq!(&content, orig);

    // list files
    let resp = client.get(format!("/api/event/{EVENT_ID}/file")).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let files = resp.into_json::<Vec<FileInfo>>().unwrap();
    assert_eq!(files.iter().len(), 1);
    assert_eq!(&files.first().unwrap().name, file_name);
    
    //delete not existing file
    let resp = client.delete(format!("/api/event/{EVENT_ID}/file/42")).dispatch();
    //if resp.status() != Status::Ok {
    //    panic!("resp: {:?}", resp.into_string().unwrap());
    //}
    assert_eq!(resp.status(), Status::NotFound);

    //delete existing file
    let resp = client.delete(format!("/api/event/{EVENT_ID}/file/{file_id}")).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

fn upload_start_list_impl(client: &Client) {
    let mut file = OpenOptions::new().read(true).open(format!("tests/{START_LIST_IOFXML3_FILE}")).unwrap();
    let mut data = vec![];
    file.read_to_end(&mut data).unwrap();

    let compressed = util::test::zip_data(&data).unwrap();
    let resp = client.post("/api/event/current/upload/startlist")
        .header(Header::new("qx-api-token", API_TOKEN))
        .header(ContentType::ZIP)
        .body(compressed)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}
#[test]
fn upload_start_list() {
    let client = create_test_server();
    upload_start_list_impl(&client);
    // get start list page
    let resp = client.get(format!("/event/{EVENT_ID}/startlist")).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn post_qe_out_change() {
    let client = create_test_server();
    upload_start_list_impl(&client);

    fn apply_change(client: &Client, change: &QERunChange) -> Vec<RunsRecord> {
        let resp = client.post("/api/event/current/qe/out/changes")
            .header(Header::new("qx-api-token", API_TOKEN))
            .header(ContentType::JSON)
            .json(&change)
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);

        // get change
        let resp = client.get(format!("/api/event/{EVENT_ID}/runs?run_id=1")).dispatch();
        resp.into_json::<Vec<RunsRecord>>().unwrap()
    }
    {
        let change = QERunChange {
            run_id: Some(1),
            si_id: Some(12345),
            ..Default::default()
        };
        let records = apply_change(&client, &change);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].si_id, 12345);
    }
    {
        let start_time = Some(QxDateTime::now());
        let change = QERunChange {
            run_id: Some(1),
            si_id: Some(0),
            last_name: Some("Foo".to_string()),
            start_time,
            ..Default::default()
        };
        let records = apply_change(&client, &change);
        assert_eq!(records.len(), 1);
        assert_eq!(&records[0].last_name, "Foo");
        assert_eq!(records[0].start_time, start_time);
    }
}
