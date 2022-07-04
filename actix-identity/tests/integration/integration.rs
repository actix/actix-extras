use std::time::Duration;

use actix_identity::{config::LogoutBehaviour, IdentityMiddleware};
use actix_web::http::StatusCode;

use crate::{fixtures::user_id, test_app::TestApp};

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

#[actix_web::test]
async fn logout_can_avoid_destroying_the_whole_session() {
    let app = TestApp::spawn_with_config(
        IdentityMiddleware::builder().logout_behaviour(LogoutBehaviour::DeleteIdentityKeys),
    );
    let user_id = user_id();

    // Log-in
    let body = app.post_login(user_id.clone()).await;
    assert_eq!(body.user_id, Some(user_id.clone()));
    assert_eq!(body.counter, 0);

    // Increment counter
    let body = app.post_increment().await;
    assert_eq!(body.user_id, Some(user_id.clone()));
    assert_eq!(body.counter, 1);

    // Log-out
    let response = app.post_logout().await;
    assert!(response.status().is_success());

    // Check the state of the counter attached to the session state
    let body = app.get_current().await;
    assert_eq!(body.user_id, None);
    // It would be 0 if the session state had been entirely lost!
    assert_eq!(body.counter, 1);
}

#[actix_web::test]
async fn user_is_logged_out_when_login_deadline_is_elapsed() {
    let login_deadline = Duration::from_millis(10);
    let app = TestApp::spawn_with_config(
        IdentityMiddleware::builder().login_deadline(Some(login_deadline)),
    );
    let user_id = user_id();

    // Log-in
    let body = app.post_login(user_id.clone()).await;
    assert_eq!(body.user_id, Some(user_id.clone()));

    // Wait for deadline to pass
    actix_web::rt::time::sleep(login_deadline * 2).await;

    let body = app.get_current().await;
    // We have been logged out!
    assert_eq!(body.user_id, None);
}

#[actix_web::test]
async fn login_deadline_does_not_log_users_out_before_their_time() {
    // 1 hour
    let login_deadline = Duration::from_secs(60 * 60);
    let app = TestApp::spawn_with_config(
        IdentityMiddleware::builder().login_deadline(Some(login_deadline)),
    );
    let user_id = user_id();

    // Log-in
    let body = app.post_login(user_id.clone()).await;
    assert_eq!(body.user_id, Some(user_id.clone()));

    let body = app.get_current().await;
    assert_eq!(body.user_id, Some(user_id));
}

#[actix_web::test]
async fn user_is_logged_out_when_visit_deadline_is_elapsed() {
    let visit_deadline = Duration::from_millis(10);
    let app = TestApp::spawn_with_config(
        IdentityMiddleware::builder().visit_deadline(Some(visit_deadline)),
    );
    let user_id = user_id();

    // Log-in
    let body = app.post_login(user_id.clone()).await;
    assert_eq!(body.user_id, Some(user_id.clone()));

    // Wait for deadline to pass
    actix_web::rt::time::sleep(visit_deadline * 2).await;

    let body = app.get_current().await;
    // We have been logged out!
    assert_eq!(body.user_id, None);
}
