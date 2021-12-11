//! Cookie based sessions. See docs for [`CookieSession`].

use crate::storage::interface::{LoadError, SaveError, SessionState, UpdateError};
use crate::storage::SessionStore;

pub struct CookieSessionStore;

#[async_trait::async_trait(?Send)]
impl SessionStore for CookieSessionStore {
    async fn load(&self, session_key: &str) -> Result<Option<SessionState>, LoadError> {
        serde_json::from_str(session_key)
            .map(Option::Some)
            .map_err(anyhow::Error::new)
            .map_err(LoadError::DeserializationError)
    }

    async fn save(&self, session_state: SessionState) -> Result<String, SaveError> {
        let session_key = serde_json::to_string(&session_state)
            .map_err(anyhow::Error::new)
            .map_err(SaveError::SerializationError)?;
        if session_key.len() > 4064 {
            return Err(SaveError::GenericError(anyhow::anyhow!("Size of the serialized session is greater than 4000 bytes, the maximum limit for cookie-based session storage.")));
        }
        Ok(session_key)
    }

    async fn update(
        &self,
        _session_key: String,
        session_state: SessionState,
    ) -> Result<String, UpdateError> {
        self.save(session_state).await.map_err(|e| match e {
            SaveError::SerializationError(e) => UpdateError::SerializationError(e),
            SaveError::GenericError(e) => UpdateError::GenericError(e),
        })
    }

    async fn delete(&self, _session_key: &str) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::web::Bytes;
    use actix_web::{test, web, App};

    #[actix_rt::test]
    async fn cookie_session() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::signed(&[0; 32]).secure(false))
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        assert!(response
            .response()
            .cookies()
            .any(|c| c.name() == "actix-session"));
    }

    #[actix_rt::test]
    async fn private_cookie() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::private(&[0; 32]).secure(false))
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        assert!(response
            .response()
            .cookies()
            .any(|c| c.name() == "actix-session"));
    }

    #[actix_rt::test]
    async fn lazy_cookie() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::signed(&[0; 32]).secure(false).lazy(true))
                .service(web::resource("/count").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "counting"
                }))
                .service(web::resource("/").to(|_ses: Session| async move { "test" })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        assert!(response.response().cookies().count() == 0);

        let request = test::TestRequest::with_uri("/count").to_request();
        let response = app.call(request).await.unwrap();

        assert!(response
            .response()
            .cookies()
            .any(|c| c.name() == "actix-session"));
    }

    #[actix_rt::test]
    async fn cookie_session_extractor() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::signed(&[0; 32]).secure(false))
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        assert!(response
            .response()
            .cookies()
            .any(|c| c.name() == "actix-session"));
    }

    #[actix_rt::test]
    async fn basics() {
        let app = test::init_service(
            App::new()
                .wrap(
                    CookieSession::signed(&[0; 32])
                        .path("/test/")
                        .name("actix-test")
                        .domain("localhost")
                        .http_only(true)
                        .same_site(SameSite::Lax)
                        .max_age(100),
                )
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                }))
                .service(web::resource("/test/").to(|ses: Session| async move {
                    let val: usize = ses.get("counter").unwrap().unwrap();
                    format!("counter: {}", val)
                })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        let cookie = response
            .response()
            .cookies()
            .find(|c| c.name() == "actix-test")
            .unwrap()
            .clone();
        assert_eq!(cookie.path().unwrap(), "/test/");

        let request = test::TestRequest::with_uri("/test/")
            .cookie(cookie)
            .to_request();
        let body = test::read_response(&app, request).await;
        assert_eq!(body, Bytes::from_static(b"counter: 100"));
    }

    #[actix_rt::test]
    async fn prolong_expiration() {
        let app = test::init_service(
            App::new()
                .wrap(CookieSession::signed(&[0; 32]).secure(false).expires_in(60))
                .service(web::resource("/").to(|ses: Session| async move {
                    let _ = ses.insert("counter", 100);
                    "test"
                }))
                .service(web::resource("/test/").to(|| async move { "no-changes-in-session" })),
        )
        .await;

        let request = test::TestRequest::get().to_request();
        let response = app.call(request).await.unwrap();
        let expires_1 = response
            .response()
            .cookies()
            .find(|c| c.name() == "actix-session")
            .expect("Cookie is set")
            .expires()
            .expect("Expiration is set")
            .datetime()
            .expect("Expiration is a datetime");

        actix_rt::time::sleep(std::time::Duration::from_secs(1)).await;

        let request = test::TestRequest::with_uri("/test/").to_request();
        let response = app.call(request).await.unwrap();
        let expires_2 = response
            .response()
            .cookies()
            .find(|c| c.name() == "actix-session")
            .expect("Cookie is set")
            .expires()
            .expect("Expiration is set")
            .datetime()
            .expect("Expiration is a datetime");

        assert!(expires_2 - expires_1 >= Duration::seconds(1));
    }
}
