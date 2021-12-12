//! Sessions for Actix Web.
//!
//! Provides a general solution for session management. Session middleware could provide different
//! implementations which could be accessed via general session API.
//!
//! This crate provides a general solution for session management and includes a cookie backend.
//! Other backend implementations can be built to use persistent or key-value stores, for example.
//!
//! In general, some session middleware, such as a [`CookieSession`] is initialized and applied.
//! To access session data, the [`Session`] extractor must be used. This extractor allows reading
//! modifying session data.
//!
//! ```no_run
//! use actix_web::{web, App, HttpServer, HttpResponse, Error};
//! use actix_session::{Session, SessionMiddleware, CookieSessionStore};
//! use actix_web::cookie::Key;
//!
//! fn index(session: Session) -> Result<&'static str, Error> {
//!     // access session data
//!     if let Some(count) = session.get::<i32>("counter")? {
//!         println!("SESSION value: {}", count);
//!         session.insert("counter", count + 1)?;
//!     } else {
//!         session.insert("counter", 1)?;
//!     }
//!
//!     Ok("Welcome!")
//! }
//!
//! // The signing key would usually be read from a configuration file/environment variables.
//! fn get_signing_key() -> Key {
//!     # use rand::distributions::Alphanumeric;
//!     # use rand::{thread_rng, Rng};
//!     # let signing_key: String = thread_rng()
//!     #     .sample_iter(&Alphanumeric)
//!     #     .take(64)
//!     #     .map(char::from)
//!     #     .collect();
//!     # Key::from(signing_key.as_bytes())
//!     // [...]
//! }
//!
//! #[actix_rt::main]
//! async fn main() -> std::io::Result<()> {
//!     let signing_key = get_signing_key();
//!     HttpServer::new(move ||
//!             App::new()
//!             // Create cookie-based session middleware
//!             .wrap(SessionMiddleware::new(CookieSessionStore::default(), signing_key.clone()))
//!             .default_service(web::to(|| HttpResponse::Ok())))
//!         .bind(("127.0.0.1", 8080))?
//!         .run()
//!         .await
//! }
//! ```

#![deny(rust_2018_idioms, nonstandard_style)]
// #![warn(missing_docs)]

pub use extractors::UserSession;
pub use middleware::{CookieContentSecurity, SessionMiddleware, SessionMiddlewareBuilder};
pub use session::{Session, SessionStatus};
#[cfg(feature = "cookie-session")]
pub use storage::CookieSessionStore;
#[cfg(feature = "redis-actor-session")]
pub use storage::RedisActorSessionStore;

mod extractors;
mod middleware;
mod session;
pub mod storage;

#[cfg(test)]
pub mod test_helpers {
    use crate::storage::SessionStore;
    use actix_web::cookie::Key;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    /// Generate a random cookie signing/encryption key.
    pub fn key() -> Key {
        let signing_key: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        Key::from(signing_key.as_bytes())
    }

    /// A ready-to-go acceptance test suite to verify that sessions behave as expected
    /// regardless of the underlying session store.
    ///
    /// `is_invalidation_supported` must be set to `true` if the backend supports
    /// "remembering" that a session has been invalidated (e.g. by logging out).
    /// It should be to `false` if the backend allows multiple cookies to be active
    /// at the same time (e.g. cookie store backend).
    pub async fn acceptance_test_suite<F, Store>(store_builder: F, is_invalidation_supported: bool)
    where
        Store: SessionStore + 'static,
        F: Fn() -> Store + Clone + Send + 'static,
    {
        acceptance_tests::complex_workflow(store_builder.clone(), is_invalidation_supported).await;
    }

    mod acceptance_tests {
        use crate::storage::SessionStore;
        use crate::test_helpers::key;
        use crate::{Session, SessionMiddleware};
        use actix_web::{
            middleware, web,
            web::{get, post, resource},
            App, HttpResponse, Result,
        };
        use serde::{Deserialize, Serialize};
        use serde_json::json;
        use time::Duration;

        pub(super) async fn complex_workflow<F, Store>(
            store_builder: F,
            is_invalidation_supported: bool,
        ) where
            Store: SessionStore + 'static,
            F: Fn() -> Store + Clone + Send + 'static,
        {
            let srv = actix_test::start(move || {
                App::new()
                    .wrap(
                        SessionMiddleware::builder(store_builder(), key())
                            .cookie_name("test-session".into())
                            .cookie_max_age(Some(time::Duration::days(7)))
                            .build(),
                    )
                    .wrap(middleware::Logger::default())
                    .service(resource("/").route(get().to(index)))
                    .service(resource("/do_something").route(post().to(do_something)))
                    .service(resource("/login").route(post().to(login)))
                    .service(resource("/logout").route(post().to(logout)))
            });

            // Step 1:  GET index
            //   - set-cookie actix-session should NOT be in response (session data is empty)
            //   - response should be: {"counter": 0, "user_id": None}
            let req_1a = srv.get("/").send();
            let mut resp_1 = req_1a.await.unwrap();
            assert!(resp_1.cookies().unwrap().is_empty());
            let result_1 = resp_1.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_1,
                IndexResponse {
                    user_id: None,
                    counter: 0
                }
            );

            // Step 2: POST to do_something
            //   - adds new session state in redis:  {"counter": 1}
            //   - set-cookie actix-session should be in response (session cookie #1)
            //   - response should be: {"counter": 1, "user_id": None}
            let req_2 = srv.post("/do_something").send();
            let mut resp_2 = req_2.await.unwrap();
            let result_2 = resp_2.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_2,
                IndexResponse {
                    user_id: None,
                    counter: 1
                }
            );
            let cookie_1 = resp_2
                .cookies()
                .unwrap()
                .clone()
                .into_iter()
                .find(|c| c.name() == "test-session")
                .unwrap();
            assert_eq!(cookie_1.max_age(), Some(Duration::days(7)));

            // Step 3:  GET index, including session cookie #1 in request
            //   - set-cookie will *not* be in response
            //   - response should be: {"counter": 1, "user_id": None}
            let req_3 = srv.get("/").cookie(cookie_1.clone()).send();
            let mut resp_3 = req_3.await.unwrap();
            assert!(resp_3.cookies().unwrap().is_empty());
            let result_3 = resp_3.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_3,
                IndexResponse {
                    user_id: None,
                    counter: 1
                }
            );

            // Step 4: POST again to do_something, including session cookie #1 in request
            //   - set-cookie will be in response (session cookie #2)
            //   - updates session state:  {"counter": 2}
            //   - response should be: {"counter": 2, "user_id": None}
            let req_4 = srv.post("/do_something").cookie(cookie_1.clone()).send();
            let mut resp_4 = req_4.await.unwrap();
            let result_4 = resp_4.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_4,
                IndexResponse {
                    user_id: None,
                    counter: 2
                }
            );
            let cookie_2 = resp_4
                .cookies()
                .unwrap()
                .clone()
                .into_iter()
                .find(|c| c.name() == "test-session")
                .unwrap();
            assert_eq!(cookie_2.max_age(), Some(Duration::days(7)));

            // Step 5: POST to login, including session cookie #2 in request
            //   - set-cookie actix-session will be in response  (session cookie #3)
            //   - updates session state: {"counter": 2, "user_id": "ferris"}
            let req_5 = srv
                .post("/login")
                .cookie(cookie_2.clone())
                .send_json(&json!({"user_id": "ferris"}));
            let mut resp_5 = req_5.await.unwrap();
            let cookie_3 = resp_5
                .cookies()
                .unwrap()
                .clone()
                .into_iter()
                .find(|c| c.name() == "test-session")
                .unwrap();
            assert_ne!(cookie_2.value(), cookie_3.value());

            let result_5 = resp_5.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_5,
                IndexResponse {
                    user_id: Some("ferris".into()),
                    counter: 2
                }
            );

            // Step 6: GET index, including session cookie #3 in request
            //   - response should be: {"counter": 2, "user_id": "ferris"}
            let req_6 = srv.get("/").cookie(cookie_3.clone()).send();
            let mut resp_6 = req_6.await.unwrap();
            let result_6 = resp_6.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_6,
                IndexResponse {
                    user_id: Some("ferris".into()),
                    counter: 2
                }
            );

            // Step 7: POST again to do_something, including session cookie #3 in request
            //   - updates session state: {"counter": 3, "user_id": "ferris"}
            //   - response should be: {"counter": 3, "user_id": "ferris"}
            let req_7 = srv.post("/do_something").cookie(cookie_3.clone()).send();
            let mut resp_7 = req_7.await.unwrap();
            let result_7 = resp_7.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_7,
                IndexResponse {
                    user_id: Some("ferris".into()),
                    counter: 3
                }
            );

            // Step 8: GET index, including session cookie #2 in request
            // If invalidation is supported, no state will be found associated to this session.
            // If invalidation is not supported, the old state will still be retrieved.
            let req_8 = srv.get("/").cookie(cookie_2.clone()).send();
            let mut resp_8 = req_8.await.unwrap();
            if is_invalidation_supported {
                assert!(resp_8.cookies().unwrap().is_empty());
                let result_8 = resp_8.json::<IndexResponse>().await.unwrap();
                assert_eq!(
                    result_8,
                    IndexResponse {
                        user_id: None,
                        counter: 0
                    }
                );
            } else {
                let result_8 = resp_8.json::<IndexResponse>().await.unwrap();
                assert_eq!(
                    result_8,
                    IndexResponse {
                        user_id: None,
                        counter: 2
                    }
                );
            }

            // Step 9: POST to logout, including session cookie #3
            //   - set-cookie actix-session will be in response with session cookie #3
            //     invalidation logic
            let req_9 = srv.post("/logout").cookie(cookie_3.clone()).send();
            let resp_9 = req_9.await.unwrap();
            let cookie_3 = resp_9
                .cookies()
                .unwrap()
                .clone()
                .into_iter()
                .find(|c| c.name() == "test-session")
                .unwrap();
            assert_eq!(0, cookie_3.max_age().map(|t| t.whole_seconds()).unwrap());

            // Step 10: GET index, including session cookie #3 in request
            //   - set-cookie actix-session should NOT be in response if invalidation is supported
            //   - response should be: {"counter": 0, "user_id": None}
            let req_10 = srv.get("/").cookie(cookie_3.clone()).send();
            let mut resp_10 = req_10.await.unwrap();
            if is_invalidation_supported {
                assert!(resp_10.cookies().unwrap().is_empty());
            }
            let result_10 = resp_10.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_10,
                IndexResponse {
                    user_id: None,
                    counter: 0
                }
            );
        }

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        pub struct IndexResponse {
            user_id: Option<String>,
            counter: i32,
        }

        async fn index(session: Session) -> Result<HttpResponse> {
            let user_id: Option<String> = session.get::<String>("user_id").unwrap();
            let counter: i32 = session
                .get::<i32>("counter")
                .unwrap_or(Some(0))
                .unwrap_or(0);

            Ok(HttpResponse::Ok().json(&IndexResponse { user_id, counter }))
        }

        async fn do_something(session: Session) -> Result<HttpResponse> {
            let user_id: Option<String> = session.get::<String>("user_id").unwrap();
            let counter: i32 = session
                .get::<i32>("counter")
                .unwrap_or(Some(0))
                .map_or(1, |inner| inner + 1);
            session.insert("counter", &counter)?;

            Ok(HttpResponse::Ok().json(&IndexResponse { user_id, counter }))
        }

        #[derive(Deserialize)]
        struct Identity {
            user_id: String,
        }

        async fn login(user_id: web::Json<Identity>, session: Session) -> Result<HttpResponse> {
            let id = user_id.into_inner().user_id;
            session.insert("user_id", &id)?;
            session.renew();

            let counter: i32 = session
                .get::<i32>("counter")
                .unwrap_or(Some(0))
                .unwrap_or(0);

            Ok(HttpResponse::Ok().json(&IndexResponse {
                user_id: Some(id),
                counter,
            }))
        }

        async fn logout(session: Session) -> Result<HttpResponse> {
            let id: Option<String> = session.get("user_id")?;

            let body = if let Some(x) = id {
                session.purge();
                format!("Logged out: {}", x)
            } else {
                "Could not log out anonymous user".to_owned()
            };

            Ok(HttpResponse::Ok().body(body))
        }
    }
}
