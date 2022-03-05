use actix_session::{storage::CookieSessionStore, Session, SessionMiddleware};
use actix_web::{
    cookie::{time::Duration, Key},
    test, web, App, Responder,
};

async fn login(session: Session) -> impl Responder {
    session.insert("user_id", "id").unwrap();
    "Logged in"
}

async fn logout(session: Session) -> impl Responder {
    session.purge();
    "Logged out"
}

#[actix_web::test]
async fn cookie_storage() -> std::io::Result<()> {
    let signing_key = Key::generate();
    let app = test::init_service(
        App::new()
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), signing_key.clone())
                    .cookie_path("/test".to_string())
                    .cookie_domain(Some("localhost".to_string()))
                    .build(),
            )
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout)),
    )
    .await;

    let login_request = test::TestRequest::post().uri("/login").to_request();
    let login_response = test::call_service(&app, login_request).await;
    let session_cookie = login_response.response().cookies().next().unwrap();
    assert_eq!(session_cookie.name(), "id");
    assert_eq!(session_cookie.path().unwrap(), "/test");
    assert!(session_cookie.secure().unwrap());
    assert!(session_cookie.http_only().unwrap());
    assert!(session_cookie.max_age().is_none());
    assert_eq!(session_cookie.domain().unwrap(), "localhost");

    let logout_request = test::TestRequest::post()
        .cookie(session_cookie)
        .uri("/logout")
        .to_request();
    let logout_response = test::call_service(&app, logout_request).await;
    let deletion_cookie = logout_response.response().cookies().next().unwrap();
    assert_eq!(deletion_cookie.name(), "id");
    assert_eq!(deletion_cookie.path().unwrap(), "/test");
    assert!(deletion_cookie.secure().is_none());
    assert!(deletion_cookie.http_only().unwrap());
    assert_eq!(deletion_cookie.max_age().unwrap(), Duration::ZERO);
    assert_eq!(deletion_cookie.domain().unwrap(), "localhost");
    Ok(())
}
