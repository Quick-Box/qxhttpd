use crate::event::START_LIST_IOFXML3_FILE;
use std::fs::OpenOptions;
use std::io::{Read};
use rocket::local::blocking::Client;
use rocket::http::{ContentType, Header, Status};
use crate::event::{EventId, EventRecord, PostedEvent};
use crate::files::FileInfo;
use crate::qxdatetime::QxDateTime;
use crate::{util};
use crate::changes::DataId;
use crate::runs::RunsRecord;

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
    let event = resp.into_json::<EventRecord>().unwrap();
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
    let event = resp.into_json::<EventRecord>().unwrap();
    assert_eq!(event.name, post_event.name);
    assert_eq!(event.place, post_event.place);
    assert_eq!(event.start_time.0, post_event.start_time);
}

#[test]
fn upload_file() {
    let client = create_test_server();

    // send file
    let file_name = "a.txt";
    let data = b"foo-bar-baz";
    let compressed_data = util::test::zip_data(data).unwrap();
    let resp = client.post(format!("/api/event/current/file?name={file_name}"))
        .header(Header::new("qx-api-token", API_TOKEN))
        .header(ContentType::ZIP)
        .body(compressed_data)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let file_id = resp.into_string().unwrap().parse::<i32>().unwrap();

    // get this file
    let resp = client.get(format!("/api/event/{EVENT_ID}/file/{file_id}")).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let content = resp.into_bytes().unwrap();
    assert_eq!(&content, data);

    // list files
    let resp = client.get(format!("/api/event/{EVENT_ID}/file")).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let files = resp.into_json::<Vec<FileInfo>>().unwrap();
    assert!(files.iter().find(|f| f.name == file_name).is_some());
    
    //delete not existing file
    let resp = client.delete(format!("/api/event/{EVENT_ID}/file/42")).dispatch();
    assert_eq!(resp.status(), Status::NotFound);

    //delete existing file
    let resp = client.delete(format!("/api/event/{EVENT_ID}/file/{file_id}")).dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // list files
    let resp = client.get(format!("/api/event/{EVENT_ID}/file")).dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let files = resp.into_json::<Vec<FileInfo>>().unwrap();
    assert!(files.iter().find(|f| f.name == file_name).is_none());
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
fn post_qe_change() {
    let client = create_test_server();
    upload_start_list_impl(&client);

    fn apply_change(client: &Client, run_id: DataId, change: &RunsRecord) -> RunsRecord {
        let resp = client.post(format!("/api/event/current/changes/run-updated?run_id={}", run_id.unwrap()))
            .header(Header::new("qx-api-token", API_TOKEN))
            .header(ContentType::JSON)
            .json(&change)
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);

        // get change
        let resp = client.get(format!("/api/event/{EVENT_ID}/runs?run_id=1")).dispatch();
        resp.into_json::<Vec<RunsRecord>>().unwrap().first().unwrap().clone()
    }
    {
        let change = RunsRecord {
            run_id: 1,
            si_id: Some(12345),
            ..Default::default()
        };
        let rec = apply_change(&client, Some(1), &change);
        assert_eq!(rec.si_id, Some(12345));
    }
    {
        let start_time = QxDateTime::now().trimmed_to_sec();
        let change = RunsRecord {
            run_id: 1,
            last_name: Some("Foo".to_string()),
            start_time: Some(start_time),
            ..Default::default()
        };
        let rec = apply_change(&client, Some(1), &change);
        assert_eq!(rec.last_name, Some("Foo".to_string()));
        assert_eq!(rec.start_time, Some(start_time));
    }
}
