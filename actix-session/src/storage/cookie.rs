//! Cookie based sessions. See docs for [`CookieSession`].

use crate::storage::interface::{LoadError, SaveError, SessionState, UpdateError};
use crate::storage::SessionStore;

#[derive(Default)]
#[non_exhaustive]
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
    use super::CookieSessionStore;
    use crate::test_helpers::acceptance_test_suite;

    #[actix_rt::test]
    async fn test_session_workflow() {
        acceptance_test_suite(|| CookieSessionStore::default(), false).await;
    }

    // TODO: review test
    // #[actix_rt::test]
    // async fn prolong_expiration() {
    //     let app = test::init_service(
    //         App::new()
    //             .wrap(CookieSession::signed(&[0; 32]).secure(false).expires_in(60))
    //             .service(web::resource("/").to(|ses: Session| async move {
    //                 let _ = ses.insert("counter", 100);
    //                 "test"
    //             }))
    //             .service(web::resource("/test/").to(|| async move { "no-changes-in-session" })),
    //     )
    //     .await;
    //
    //     let request = test::TestRequest::get().to_request();
    //     let response = app.call(request).await.unwrap();
    //     let expires_1 = response
    //         .response()
    //         .cookies()
    //         .find(|c| c.name() == "actix-session")
    //         .expect("Cookie is set")
    //         .expires()
    //         .expect("Expiration is set")
    //         .datetime()
    //         .expect("Expiration is a datetime");
    //
    //     actix_rt::time::sleep(std::time::Duration::from_secs(1)).await;
    //
    //     let request = test::TestRequest::with_uri("/test/").to_request();
    //     let response = app.call(request).await.unwrap();
    //     let expires_2 = response
    //         .response()
    //         .cookies()
    //         .find(|c| c.name() == "actix-session")
    //         .expect("Cookie is set")
    //         .expires()
    //         .expect("Expiration is set")
    //         .datetime()
    //         .expect("Expiration is a datetime");
    //
    //     assert!(expires_2 - expires_1 >= Duration::seconds(1));
    // }
}
