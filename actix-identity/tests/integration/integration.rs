use crate::fixtures::user_id;
use crate::test_app::TestApp;
use actix_web::http::StatusCode;

#[actix_web::test]
async fn opaque_401_is_returned_for_unauthenticated_users() {
    let app = TestApp::spawn();

    let response = app.get_identity_required().await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.bytes().await.unwrap().is_empty());
}

#[actix_web::test]
async fn login_works() {
    let app = TestApp::spawn();
    let user_id = user_id();

    // Log-in
    let body = app.post_login(user_id.clone()).await;
    assert_eq!(body.user_id, Some(user_id.clone()));

    // Access identity-restricted route successfully
    let response = app.get_identity_required().await;
    assert!(response.status().is_success());
}

#[actix_web::test]
async fn logging_in_again_replaces_the_current_identity() {
    let app = TestApp::spawn();
    let first_user_id = user_id();
    let second_user_id = user_id();

    // Log-in
    let body = app.post_login(first_user_id.clone()).await;
    assert_eq!(body.user_id, Some(first_user_id.clone()));

    // Log-in again
    let body = app.post_login(second_user_id.clone()).await;
    assert_eq!(body.user_id, Some(second_user_id.clone()));

    let body = app.get_current().await;
    assert_eq!(body.user_id, Some(second_user_id.clone()));
}

#[actix_web::test]
async fn session_key_is_renewed_on_login() {
    let app = TestApp::spawn();
    let user_id = user_id();

    // Create an anonymous session
    let body = app.post_increment().await;
    assert_eq!(body.user_id, None);
    assert_eq!(body.counter, 1);
    assert_eq!(body.session_status, "changed");

    // Log-in
    let body = app.post_login(user_id.clone()).await;
    assert_eq!(body.user_id, Some(user_id.clone()));
    assert_eq!(body.counter, 1);
    assert_eq!(body.session_status, "renewed");
}

#[actix_web::test]
async fn logout_works() {
    let app = TestApp::spawn();
    let user_id = user_id();

    // Log-in
    let body = app.post_login(user_id.clone()).await;
    assert_eq!(body.user_id, Some(user_id.clone()));

    // Log-out
    let response = app.post_logout().await;
    assert!(response.status().is_success());

    // Try to access identity-restricted route
    let response = app.get_identity_required().await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
