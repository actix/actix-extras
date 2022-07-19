use std::collections::HashMap;
use std::convert::TryInto;

use actix_session::storage::{LoadError, SaveError, SessionKey, SessionStore, UpdateError};
use actix_session::{Session, SessionMiddleware};
use actix_web::body::MessageBody;
use actix_web::http::StatusCode;
use actix_web::{
    cookie::{time::Duration, Key},
    dev::Service,
    test, web, App, Responder,
};
use anyhow::Error;

#[actix_web::test]
async fn errors_are_opaque() {
    let signing_key = Key::generate();
    let app = test::init_service(
        App::new()
            .wrap(SessionMiddleware::new(MockStore, signing_key.clone()))
            .route("/create_session", web::post().to(create_session))
            .route(
                "/load_session_with_error",
                web::post().to(load_session_with_error),
            ),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/create_session")
        .to_request();
    let response = test::call_service(&app, req).await;
    let session_cookie = response.response().cookies().next().unwrap();

    let req = test::TestRequest::post()
        .cookie(session_cookie)
        .uri("/load_session_with_error")
        .to_request();
    let response = app.call(req).await.unwrap_err().error_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(response.into_body().try_into_bytes().unwrap().is_empty());
}

struct MockStore;

#[async_trait::async_trait(?Send)]
impl SessionStore for MockStore {
    async fn load(
        &self,
        _session_key: &SessionKey,
    ) -> Result<Option<HashMap<String, String>>, LoadError> {
        Err(LoadError::Other(anyhow::anyhow!(
            "My error full of implementation details"
        )))
    }

    async fn save(
        &self,
        _session_state: HashMap<String, String>,
        _ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        Ok("random_value".to_string().try_into().unwrap())
    }

    async fn update(
        &self,
        _session_key: SessionKey,
        _session_state: HashMap<String, String>,
        _ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        todo!()
    }

    async fn update_ttl(&self, _session_key: &SessionKey, _ttl: &Duration) -> Result<(), Error> {
        todo!()
    }

    async fn delete(&self, _session_key: &SessionKey) -> Result<(), Error> {
        todo!()
    }
}

async fn create_session(session: Session) -> impl Responder {
    session.insert("user_id", "id").unwrap();
    "Created"
}

async fn load_session_with_error(_session: Session) -> impl Responder {
    "Loaded"
}
