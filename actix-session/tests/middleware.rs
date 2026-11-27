use actix_session::{storage::CookieSessionStore, Session, SessionMiddleware};
use actix_web::{
    cookie::{time::Duration, Key, SameSite},
    dev::ServiceResponse,
    http::header::SET_COOKIE,
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

#[tokio::test(flavor = "local")]
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
    assert!(deletion_cookie.secure().unwrap());
    assert!(deletion_cookie.http_only().unwrap());
    assert_eq!(deletion_cookie.max_age().unwrap(), Duration::ZERO);
    assert_eq!(deletion_cookie.domain().unwrap(), "localhost");
    Ok(())
}

fn set_cookie_headers(res: &ServiceResponse) -> Vec<String> {
    res.headers()
        .get_all(SET_COOKIE)
        .map(|val| val.to_str().unwrap().to_owned())
        .collect()
}

fn has_attribute(set_cookie: &str, attribute: &str) -> bool {
    set_cookie
        .split(';')
        .skip(1)
        .any(|attr| attr.trim().eq_ignore_ascii_case(attribute))
}

#[tokio::test(flavor = "local")]
async fn cookie_not_partitioned_by_default() -> std::io::Result<()> {
    let app = test::init_service(
        App::new()
            .wrap(SessionMiddleware::new(
                CookieSessionStore::default(),
                Key::generate(),
            ))
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout)),
    )
    .await;

    let login_request = test::TestRequest::post().uri("/login").to_request();
    let login_response = test::call_service(&app, login_request).await;
    let set_cookie = set_cookie_headers(&login_response);
    assert_eq!(set_cookie.len(), 1);
    assert!(!has_attribute(&set_cookie[0], "Partitioned"));

    let session_cookie = login_response.response().cookies().next().unwrap();
    let logout_request = test::TestRequest::post()
        .cookie(session_cookie)
        .uri("/logout")
        .to_request();
    let logout_response = test::call_service(&app, logout_request).await;
    let set_cookie = set_cookie_headers(&logout_response);
    assert_eq!(set_cookie.len(), 1);
    assert!(!has_attribute(&set_cookie[0], "Partitioned"));

    Ok(())
}

#[tokio::test(flavor = "local")]
async fn cookie_partitioned() -> std::io::Result<()> {
    let app = test::init_service(
        App::new()
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
                    .cookie_same_site(SameSite::None)
                    .cookie_partitioned(true)
                    .build(),
            )
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout)),
    )
    .await;

    let login_request = test::TestRequest::post().uri("/login").to_request();
    let login_response = test::call_service(&app, login_request).await;
    let set_cookie = set_cookie_headers(&login_response);
    assert_eq!(set_cookie.len(), 1);
    assert!(has_attribute(&set_cookie[0], "Partitioned"));
    assert!(has_attribute(&set_cookie[0], "Secure"));

    let session_cookie = login_response.response().cookies().next().unwrap();
    assert_eq!(session_cookie.name(), "id");
    assert_eq!(session_cookie.same_site(), Some(SameSite::None));

    // removal cookie must be partitioned to match the session cookie
    let logout_request = test::TestRequest::post()
        .cookie(session_cookie)
        .uri("/logout")
        .to_request();
    let logout_response = test::call_service(&app, logout_request).await;
    let set_cookie = set_cookie_headers(&logout_response);
    assert_eq!(set_cookie.len(), 1);
    assert!(has_attribute(&set_cookie[0], "Partitioned"));
    assert!(has_attribute(&set_cookie[0], "Secure"));

    let deletion_cookie = logout_response.response().cookies().next().unwrap();
    assert_eq!(deletion_cookie.max_age().unwrap(), Duration::ZERO);

    Ok(())
}

#[tokio::test(flavor = "local")]
async fn cookie_partitioned_implies_secure() -> std::io::Result<()> {
    let app = test::init_service(
        App::new()
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
                    .cookie_secure(false)
                    .cookie_partitioned(true)
                    .build(),
            )
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout)),
    )
    .await;

    let login_request = test::TestRequest::post().uri("/login").to_request();
    let login_response = test::call_service(&app, login_request).await;
    let set_cookie = set_cookie_headers(&login_response);
    assert_eq!(set_cookie.len(), 1);
    assert!(has_attribute(&set_cookie[0], "Partitioned"));
    assert!(has_attribute(&set_cookie[0], "Secure"));

    let session_cookie = login_response.response().cookies().next().unwrap();
    let logout_request = test::TestRequest::post()
        .cookie(session_cookie)
        .uri("/logout")
        .to_request();
    let logout_response = test::call_service(&app, logout_request).await;
    let set_cookie = set_cookie_headers(&logout_response);
    assert_eq!(set_cookie.len(), 1);
    assert!(has_attribute(&set_cookie[0], "Partitioned"));
    assert!(has_attribute(&set_cookie[0], "Secure"));

    Ok(())
}

#[tokio::test(flavor = "local")]
async fn cookie_partitioned_with_large_session_state() -> std::io::Result<()> {
    async fn large_login(session: Session) -> impl Responder {
        session.insert("state", "x".repeat(2048)).unwrap();
        "Logged in"
    }

    async fn state(session: Session) -> impl Responder {
        session
            .get::<String>("state")
            .unwrap()
            .unwrap()
            .len()
            .to_string()
    }

    let app = test::init_service(
        App::new()
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
                    .cookie_partitioned(true)
                    .build(),
            )
            .route("/login", web::post().to(large_login))
            .route("/state", web::get().to(state)),
    )
    .await;

    let login_request = test::TestRequest::post().uri("/login").to_request();
    let login_response = test::call_service(&app, login_request).await;
    let set_cookie = set_cookie_headers(&login_response);
    assert_eq!(set_cookie.len(), 1);
    assert!(set_cookie[0].ends_with("; Partitioned"));

    let session_cookie = login_response.response().cookies().next().unwrap();
    let state_request = test::TestRequest::get()
        .cookie(session_cookie)
        .uri("/state")
        .to_request();
    let body = test::call_and_read_body(&app, state_request).await;
    assert_eq!(body, "2048");

    Ok(())
}
