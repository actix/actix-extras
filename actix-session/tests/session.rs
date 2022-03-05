use actix_session::{SessionExt, SessionStatus};
use actix_web::{test, HttpResponse};

#[actix_web::test]
async fn session() {
    let req = test::TestRequest::default().to_srv_request();
    let session = req.get_session();
    session.insert("key", "value").unwrap();
    let res = session.get::<String>("key").unwrap();
    assert_eq!(res, Some("value".to_string()));

    session.insert("key2", "value2").unwrap();
    session.remove("key");

    let res = req.into_response(HttpResponse::Ok().finish());
    let state: Vec<_> = res.get_session().entries().clone().into_iter().collect();
    assert_eq!(
        state.as_slice(),
        [("key2".to_string(), "\"value2\"".to_string())]
    );
}

#[actix_web::test]
async fn get_session() {
    let req = test::TestRequest::default().to_srv_request();

    let session = req.get_session();
    session.insert("key", true).unwrap();
    let res = session.get("key").unwrap();
    assert_eq!(res, Some(true));
}

#[actix_web::test]
async fn get_session_from_request_head() {
    let req = test::TestRequest::default().to_srv_request();

    let session = req.get_session();
    session.insert("key", 10).unwrap();
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
    let req = test::TestRequest::default().to_srv_request();
    let session = req.get_session();
    session.insert("test_str", "val").unwrap();
    session.insert("test_str", 1).unwrap();
    let map = session.entries();
    map.contains_key("test_str");
    map.contains_key("test_num");
}
