//! Sessions management for `actix-web` applications.
//!
//! To start using sessions in your `actix-web` application you must register [`SessionMiddleware`] as a middleware on your `actix-web`'s `App`:
//!
//! ```no_run
//! use actix_web::{web, App, HttpServer, HttpResponse, Error};
//! use actix_session::{Session, SessionMiddleware, storage::RedisActorSessionStore};
//! use actix_web::cookie::Key;
//!
//! // The secret key would usually be read from a configuration file/environment variables.
//! fn get_secret_key() -> Key {
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
//!     let secret_key = get_secret_key();
//!     let redis_connection_string = "127.0.0.1:6379";
//!     HttpServer::new(move ||
//!             App::new()
//!             // Add session management to your application using Redis for session state storage
//!             .wrap(
//!                 SessionMiddleware::new(
//!                     RedisActorSessionStore::new(redis_connection_string),
//!                     secret_key.clone()
//!                 )
//!             )
//!             .default_service(web::to(|| HttpResponse::Ok())))
//!         .bind(("127.0.0.1", 8080))?
//!         .run()
//!         .await
//! }
//! ```
//!
//! The session state can be accessed and modified by your request handlers using the [`Session`] extractor.
//!
//! ```no_run
//! use actix_web::Error;
//! use actix_session::Session;
//!
//! fn index(session: Session) -> Result<&'static str, Error> {
//!     // Access the session state
//!     if let Some(count) = session.get::<i32>("counter")? {
//!         println!("SESSION value: {}", count);
//!         // Modify the session state
//!         session.insert("counter", count + 1)?;
//!     } else {
//!         session.insert("counter", 1)?;
//!     }
//!
//!     Ok("Welcome!")
//! }
//! ```

#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(missing_docs)]

pub use extractors::SessionExt;
pub use middleware::{CookieContentSecurity, SessionMiddleware, SessionMiddlewareBuilder};
pub use session::{Session, SessionStatus};

mod extractors;
mod middleware;
mod session;
pub mod storage;

#[cfg(test)]
pub mod test_helpers {
    use actix_web::cookie::Key;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    use crate::storage::SessionStore;
    use crate::CookieContentSecurity;

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
        for policy in vec![
            CookieContentSecurity::Signed,
            CookieContentSecurity::Private,
        ] {
            println!("Using {:?} as cookie content security policy.", policy);
            acceptance_tests::basic_workflow(store_builder.clone(), policy).await;
            acceptance_tests::expiration_is_refreshed_on_changes(store_builder.clone(), policy)
                .await;
            acceptance_tests::complex_workflow(
                store_builder.clone(),
                is_invalidation_supported,
                policy,
            )
            .await;
        }
    }

    mod acceptance_tests {
        use actix_web::web::Bytes;
        use actix_web::{dev::Service, test, App};
        use actix_web::{
            middleware, web,
            web::{get, post, resource},
            HttpResponse, Result,
        };
        use serde::{Deserialize, Serialize};
        use serde_json::json;
        use time::Duration;

        use crate::middleware::SessionLength;
        use crate::storage::SessionStore;
        use crate::test_helpers::key;
        use crate::{CookieContentSecurity, Session, SessionMiddleware};

        pub(super) async fn basic_workflow<F, Store>(
            store_builder: F,
            policy: CookieContentSecurity,
        ) where
            Store: SessionStore + 'static,
            F: Fn() -> Store + Clone + Send + 'static,
        {
            let app = test::init_service(
                App::new()
                    .wrap(
                        SessionMiddleware::builder(store_builder(), key())
                            .cookie_path("/test/".into())
                            .cookie_name("actix-test".into())
                            .cookie_domain(Some("localhost".into()))
                            .cookie_content_security(policy)
                            .session_length(SessionLength::Predetermined {
                                max_session_length: Some(time::Duration::seconds(100)),
                            })
                            .build(),
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

        pub(super) async fn expiration_is_refreshed_on_changes<F, Store>(
            store_builder: F,
            policy: CookieContentSecurity,
        ) where
            Store: SessionStore + 'static,
            F: Fn() -> Store + Clone + Send + 'static,
        {
            let app = test::init_service(
                App::new()
                    .wrap(
                        SessionMiddleware::builder(store_builder(), key())
                            .cookie_content_security(policy)
                            .session_length(SessionLength::Predetermined {
                                max_session_length: Some(time::Duration::seconds(60)),
                            })
                            .build(),
                    )
                    .service(web::resource("/").to(|ses: Session| async move {
                        let _ = ses.insert("counter", 100);
                        "test"
                    }))
                    .service(web::resource("/test/").to(|| async move { "no-changes-in-session" })),
            )
            .await;

            let request = test::TestRequest::get().to_request();
            let response = app.call(request).await.unwrap();
            let cookie_1 = response
                .response()
                .cookies()
                .find(|c| c.name() == "id")
                .expect("Cookie is set");
            assert_eq!(cookie_1.max_age(), Some(Duration::seconds(60)));

            let request = test::TestRequest::with_uri("/test/").to_request();
            let response = app.call(request).await.unwrap();
            assert!(response.response().cookies().collect::<Vec<_>>().is_empty());

            let request = test::TestRequest::get().to_request();
            let response = app.call(request).await.unwrap();
            let cookie_2 = response
                .response()
                .cookies()
                .find(|c| c.name() == "id")
                .expect("Cookie is set");
            assert_eq!(cookie_2.max_age(), Some(Duration::seconds(60)));
        }

        pub(super) async fn complex_workflow<F, Store>(
            store_builder: F,
            is_invalidation_supported: bool,
            policy: CookieContentSecurity,
        ) where
            Store: SessionStore + 'static,
            F: Fn() -> Store + Clone + Send + 'static,
        {
            let srv = actix_test::start(move || {
                App::new()
                    .wrap(
                        SessionMiddleware::builder(store_builder(), key())
                            .cookie_name("test-session".into())
                            .cookie_content_security(policy)
                            .session_length(SessionLength::Predetermined {
                                max_session_length: Some(time::Duration::days(7)),
                            })
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
