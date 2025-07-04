use crate::changes::{rocket_uri_macro_api_changes_get, ChangeData, ChangesRecord};
use crate::changes::rocket_uri_macro_add_run_update_request_change;
use crate::changes::rocket_uri_macro_api_changes_delete;
use crate::event::{START_LIST_IOFXML3_FILE, DEMO_API_TOKEN, TEST_SESSION_ID};
use std::fs::OpenOptions;
use std::io::{Read};
use rocket::local::blocking::Client;
use rocket::http::{ContentType, Cookie, Header, Status};
use crate::event::{EventId, EventRecord, PostedEvent};
use crate::files::FileInfo;
use crate::qxdatetime::QxDateTime;
use crate::{util};
use crate::auth::QX_SESSION_ID;
use crate::changes::DataId;
use crate::runs::RunsRecord;

const EVENT_ID: EventId = 1;

fn create_test_server() -> Client {
    let rocket = super::rocket()
        // doesn't work, don't know why
        //.attach(rocket::fairing::AdHoc::on_ignite("Secret Key", |rocket| async {
        //    rocket.manage(rocket::Config {
        //        secret_key: rocket::config::SecretKey::generate().unwrap(), // or load your own
        //        ..Default::default()
        //    })
        //}))
        ;
    let client = Client::tracked(rocket).unwrap();
    // doesn't work, don't know why
    //client.cookies().add_private(Cookie::build((QX_SESSION_ID, TEST_SESSION_ID))
    //    .path("/")
    //    .same_site(SameSite::Lax));
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
        .header(Header::new("qx-api-token", DEMO_API_TOKEN))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.content_type(), Some(ContentType::JSON));
    let event = resp.into_json::<EventRecord>().unwrap();
    assert_eq!(event.id, 1);

    let dt = QxDateTime::now().trimmed_to_sec();
    let post_event = PostedEvent {
        name: "Foo".to_string(),
        stage: 2,
        stage_count: 3,
        place: "Bar".to_string(),
        start_time: dt.0,
    };
    let resp = client.post("/api/event/current")
        .header(Header::new("qx-api-token", DEMO_API_TOKEN))
        .json(&post_event)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    let resp = client.get("/api/event/current")
        .header(Header::new("qx-api-token", DEMO_API_TOKEN))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.content_type(), Some(ContentType::JSON));
    let event = resp.into_json::<EventRecord>().unwrap();
    assert_eq!(event.name, post_event.name);
    assert_eq!(event.place, post_event.place);
    assert_eq!(event.start_time.0, post_event.start_time);
    assert_eq!(event.stage, post_event.stage);
    assert_eq!(event.stage_count, post_event.stage_count);
}

#[test]
fn upload_file() {
    let client = create_test_server();

    // send file
    let file_name = "a.txt";
    let data = b"foo-bar-baz";
    let compressed_data = util::test::zip_data(data).unwrap();
    let resp = client.post(format!("/api/event/current/file?name={file_name}"))
        .header(Header::new("qx-api-token", DEMO_API_TOKEN))
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

fn upload_test_file(client: &Client, file_name: &str) {
    let mut file = OpenOptions::new().read(true).open(format!("tests/{file_name}")).unwrap();
    let mut data = vec![];
    file.read_to_end(&mut data).unwrap();

    let compressed = util::test::zip_data(&data).unwrap();
    let resp = client.post(format!("/api/event/current/file?name={file_name}"))
        .header(Header::new("qx-api-token", DEMO_API_TOKEN))
        .header(ContentType::ZIP)
        .body(compressed)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}
fn upload_start_list(client: &Client) {
    upload_test_file(&client, START_LIST_IOFXML3_FILE);
}
#[test]
fn test_upload_start_list() {
    let client = create_test_server();
    upload_start_list(&client);
    // get start list page
    let resp = client.get(format!("/event/{EVENT_ID}/startlist")).dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn add_start_list_change_request() {
    let client = create_test_server();

    const RUN_ID: i64 = 1;
    let run_change = RunsRecord{
        run_id: RUN_ID,
        class_name: None,
        registration: None,
        first_name: None,
        last_name: None,
        si_id: Some(1234),
        start_time: None,
        check_time: None,
        finish_time: None,
    };

    // create run change request
    let resp = client.post(uri!(add_run_update_request_change(event_id = EVENT_ID, data_id = Some(RUN_ID), note = Some("foo"))))
        .cookie(Cookie::build((QX_SESSION_ID, TEST_SESSION_ID)))
        .header(ContentType::JSON)
        .json(&run_change)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let change_id = resp.into_json::<i64>().unwrap();

    // read it back
    let resp = client.get(uri!(
        api_changes_get(
            event_id = EVENT_ID,
            data_type = None::<&str>,
            status = None::<&str>,
            from_id = Some(change_id),
            limit = Some(1)
        )))
        .cookie(Cookie::build((QX_SESSION_ID, TEST_SESSION_ID)))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let change = resp.into_json::<Vec<ChangesRecord>>().unwrap().first().unwrap().clone();
    assert_eq!(change.note, Some("foo".to_string()));
    if let ChangeData::RunUpdateRequest(change) = change.data {
        assert_eq!(change.si_id, Some(1234));
    } else {
        panic!("unexpected change type");
    }

    // delete it
    let resp = client.delete(uri!(
        api_changes_delete(
            event_id = EVENT_ID,
            change_id = change_id,
        )))
        .cookie(Cookie::build((QX_SESSION_ID, TEST_SESSION_ID)))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);

    // read that is not existent anymore
    let resp = client.get(uri!(
        api_changes_get(
            event_id = EVENT_ID,
            data_type = None::<&str>,
            status = None::<&str>,
            from_id = Some(change_id),
            limit = Some(1)
        )))
        .cookie(Cookie::build((QX_SESSION_ID, TEST_SESSION_ID)))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let changes = resp.into_json::<Vec<ChangesRecord>>().unwrap().clone();
    assert!(changes.is_empty());
}

#[test]
fn post_qe3_change() {
    let client = create_test_server();
    upload_start_list(&client);

    fn apply_change_in_qe3(client: &Client, run_id: DataId, change: &RunsRecord) -> RunsRecord {
        let resp = client.post(format!("/api/event/current/changes/run-updated?run_id={}", run_id.unwrap()))
            .header(Header::new("qx-api-token", DEMO_API_TOKEN))
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
        let rec = apply_change_in_qe3(&client, Some(1), &change);
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
        let rec = apply_change_in_qe3(&client, Some(1), &change);
        assert_eq!(rec.last_name, Some("Foo".to_string()));
        assert_eq!(rec.start_time, Some(start_time));
    }
}
