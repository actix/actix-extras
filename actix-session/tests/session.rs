use actix_session::{Session, SessionStatus, UserSession};
use actix_web::{test, HttpResponse};

#[actix_web::test]
async fn session() {
    let mut req = test::TestRequest::default().to_srv_request();

    Session::set_session(
        &mut req,
        vec![("key".to_string(), serde_json::to_string("value").unwrap())],
    );
    let session = req.get_session();
    let res = session.get::<String>("key").unwrap();
    assert_eq!(res, Some("value".to_string()));

    session.insert("key2", "value2").unwrap();
    session.remove("key");

    let mut res = req.into_response(HttpResponse::Ok().finish());
    let (_status, state) = Session::get_changes(&mut res);
    let changes: Vec<_> = state.collect();
    assert_eq!(changes, [("key2".to_string(), "\"value2\"".to_string())]);
}

#[actix_web::test]
async fn get_session() {
    let mut req = test::TestRequest::default().to_srv_request();

    Session::set_session(
        &mut req,
        vec![("key".to_string(), serde_json::to_string(&true).unwrap())],
    );

    let session = req.get_session();
    let res = session.get("key").unwrap();
    assert_eq!(res, Some(true));
}

#[actix_web::test]
async fn get_session_from_request_head() {
    let mut req = test::TestRequest::default().to_srv_request();

    Session::set_session(
        &mut req,
        vec![("key".to_string(), serde_json::to_string(&10).unwrap())],
    );

    let session = req.head_mut().get_session();
    let res = session.get::<u32>("key").unwrap();
    assert_eq!(res, Some(10));
}

#[actix_web::test]
async fn purge_session() {
    let req = test::TestRequest::default().to_srv_request();
    let session = req.get_session();
    assert_eq!(session.status(), SessionStatus::Unchanged);
    session.purge();
    assert_eq!(session.status(), SessionStatus::Purged);
}

#[actix_web::test]
async fn renew_session() {
    let req = test::TestRequest::default().to_srv_request();
    let session = req.get_session();
    assert_eq!(session.status(), SessionStatus::Unchanged);
    session.renew();
    assert_eq!(session.status(), SessionStatus::Renewed);
}

#[actix_web::test]
async fn session_entries() {
    let mut req = test::TestRequest::default().to_srv_request();
    Session::set_session(
        &mut req,
        vec![
            ("test_str".to_string(), "val".into()),
            ("test_num".to_string(), "1".into()),
        ],
    );

    let session = req.get_session();
    let map = session.entries();
    map.contains_key("test_str");
    map.contains_key("test_num");
}
