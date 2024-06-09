/*!
Session management for Actix Web.

The HTTP protocol, at a first glance, is stateless: the client sends a request, the server
parses its content, performs some processing and returns a response. The outcome is only
influenced by the provided inputs (i.e. the request content) and whatever state the server
queries while performing its processing.

Stateless systems are easier to reason about, but they are not quite as powerful as we need them
to be - e.g. how do you authenticate a user? The user would be forced to authenticate **for
every single request**. That is, for example, how 'Basic' Authentication works. While it may
work for a machine user (i.e. an API client), it is impractical for a person—you do not want a
login prompt on every single page you navigate to!

There is a solution - **sessions**. Using sessions the server can attach state to a set of
requests coming from the same client. They are built on top of cookies - the server sets a
cookie in the HTTP response (`Set-Cookie` header), the client (e.g. the browser) will store the
cookie and play it back to the server when sending new requests (using the `Cookie` header).

We refer to the cookie used for sessions as a **session cookie**. Its content is called
**session key** (or **session ID**), while the state attached to the session is referred to as
**session state**.

`actix-session` provides an easy-to-use framework to manage sessions in applications built on
top of Actix Web. [`SessionMiddleware`] is the middleware underpinning the functionality
provided by `actix-session`; it takes care of all the session cookie handling and instructs the
**storage backend** to create/delete/update the session state based on the operations performed
against the active [`Session`].

`actix-session` provides some built-in storage backends: ([`CookieSessionStore`],
[`RedisSessionStore`], and [`RedisActorSessionStore`]) - you can create a custom storage backend
by implementing the [`SessionStore`] trait.

Further reading on sessions:
- [RFC 6265](https://datatracker.ietf.org/doc/html/rfc6265);
- [OWASP's session management cheat-sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html).

# Getting started
To start using sessions in your Actix Web application you must register [`SessionMiddleware`]
as a middleware on your `App`:

```no_run
use actix_web::{web, App, HttpServer, HttpResponse, Error};
use actix_session::{Session, SessionMiddleware, storage::RedisSessionStore};
use actix_web::cookie::Key;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // When using `Key::generate()` it is important to initialize outside of the
    // `HttpServer::new` closure. When deployed the secret key should be read from a
    // configuration file or environment variables.
    let secret_key = Key::generate();

    let redis_store = RedisSessionStore::new("redis://127.0.0.1:6379")
        .await
        .unwrap();

    HttpServer::new(move ||
            App::new()
            // Add session management to your application using Redis for session state storage
            .wrap(
                SessionMiddleware::new(
                    redis_store.clone(),
                    secret_key.clone(),
                )
            )
            .default_service(web::to(|| HttpResponse::Ok())))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
```

The session state can be accessed and modified by your request handlers using the [`Session`]
extractor. Note that this doesn't work in the stream of a streaming response.

```no_run
use actix_web::Error;
use actix_session::Session;

fn index(session: Session) -> Result<&'static str, Error> {
    // access the session state
    if let Some(count) = session.get::<i32>("counter")? {
        println!("SESSION value: {}", count);
        // modify the session state
        session.insert("counter", count + 1)?;
    } else {
        session.insert("counter", 1)?;
    }

    Ok("Welcome!")
}
```

# Choosing A Backend

By default, `actix-session` does not provide any storage backend to retrieve and save the state
attached to your sessions. You can enable:

- a purely cookie-based "backend", [`CookieSessionStore`], using the `cookie-session` feature
  flag.

  ```toml
  [dependencies]
  # ...
  actix-session = { version = "...", features = ["cookie-session"] }
  ```

- a Redis-based backend via [`redis-rs`](https://docs.rs/redis-rs), [`RedisSessionStore`], using
  the `redis-session` feature flag.

  ```toml
  [dependencies]
  # ...
  actix-session = { version = "...", features = ["redis-session"] }
  ```

  Add the `redis-session-native-tls` feature flag if you want to connect to Redis using a secured
  connection (via the `native-tls` crate):

  ```toml
  [dependencies]
  # ...
  actix-session = { version = "...", features = ["redis-session-native-tls"] }
  ```

  If you instead prefer depending on `rustls`, use the `redis-session-rustls` feature flag:

  ```toml
  [dependencies]
  # ...
  actix-session = { version = "...", features = ["redis-session-rustls"] }
  ```

You can implement your own session storage backend using the [`SessionStore`] trait.

[`SessionStore`]: storage::SessionStore
[`CookieSessionStore`]: storage::CookieSessionStore
[`RedisSessionStore`]: storage::RedisSessionStore
[`RedisActorSessionStore`]: storage::RedisActorSessionStore
*/

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(future_incompatible, missing_docs)]
#![doc(html_logo_url = "https://actix.rs/img/logo.png")]
#![doc(html_favicon_url = "https://actix.rs/favicon.ico")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

pub mod config;
mod middleware;
mod session;
mod session_ext;
pub mod storage;

pub use self::{
    middleware::SessionMiddleware,
    session::{Session, SessionGetError, SessionInsertError, SessionStatus},
    session_ext::SessionExt,
};

#[cfg(test)]
pub mod test_helpers {
    use actix_web::cookie::Key;

    use crate::{config::CookieContentSecurity, storage::SessionStore};

    /// Generate a random cookie signing/encryption key.
    pub fn key() -> Key {
        Key::generate()
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
        for policy in &[
            CookieContentSecurity::Signed,
            CookieContentSecurity::Private,
        ] {
            println!("Using {policy:?} as cookie content security policy.");
            acceptance_tests::basic_workflow(store_builder.clone(), *policy).await;
            acceptance_tests::expiration_is_refreshed_on_changes(store_builder.clone(), *policy)
                .await;
            acceptance_tests::expiration_is_always_refreshed_if_configured_to_refresh_on_every_request(
                store_builder.clone(),
                *policy,
            )
            .await;
            acceptance_tests::complex_workflow(
                store_builder.clone(),
                is_invalidation_supported,
                *policy,
            )
            .await;
            acceptance_tests::guard(store_builder.clone(), *policy).await;
        }
    }

    mod acceptance_tests {
        use actix_web::{
            cookie::time,
            dev::{Service, ServiceResponse},
            guard, middleware, test,
            web::{self, get, post, resource, Bytes},
            App, HttpResponse, Result,
        };
        use serde::{Deserialize, Serialize};
        use serde_json::json;

        use crate::{
            config::{CookieContentSecurity, PersistentSession, TtlExtensionPolicy},
            storage::SessionStore,
            test_helpers::key,
            Session, SessionExt, SessionMiddleware,
        };

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
                            .session_lifecycle(
                                PersistentSession::default()
                                    .session_ttl(time::Duration::seconds(100)),
                            )
                            .build(),
                    )
                    .service(web::resource("/").to(|ses: Session| async move {
                        let _ = ses.insert("counter", 100);
                        "test"
                    }))
                    .service(web::resource("/test/").to(|ses: Session| async move {
                        let val: usize = ses.get("counter").unwrap().unwrap();
                        format!("counter: {val}")
                    })),
            )
            .await;

            let request = test::TestRequest::get().to_request();
            let response = app.call(request).await.unwrap();
            let cookie = response.get_cookie("actix-test").unwrap().clone();
            assert_eq!(cookie.path().unwrap(), "/test/");

            let request = test::TestRequest::with_uri("/test/")
                .cookie(cookie)
                .to_request();
            let body = test::call_and_read_body(&app, request).await;
            assert_eq!(body, Bytes::from_static(b"counter: 100"));
        }

        pub(super) async fn expiration_is_always_refreshed_if_configured_to_refresh_on_every_request<
            F,
            Store,
        >(
            store_builder: F,
            policy: CookieContentSecurity,
        ) where
            Store: SessionStore + 'static,
            F: Fn() -> Store + Clone + Send + 'static,
        {
            let session_ttl = time::Duration::seconds(60);
            let app = test::init_service(
                App::new()
                    .wrap(
                        SessionMiddleware::builder(store_builder(), key())
                            .cookie_content_security(policy)
                            .session_lifecycle(
                                PersistentSession::default()
                                    .session_ttl(session_ttl)
                                    .session_ttl_extension_policy(
                                        TtlExtensionPolicy::OnEveryRequest,
                                    ),
                            )
                            .build(),
                    )
                    .service(web::resource("/").to(|ses: Session| async move {
                        let _ = ses.insert("counter", 100);
                        "test"
                    }))
                    .service(web::resource("/test/").to(|| async move { "no-changes-in-session" })),
            )
            .await;

            // Create session
            let request = test::TestRequest::get().to_request();
            let response = app.call(request).await.unwrap();
            let cookie_1 = response.get_cookie("id").expect("Cookie is set");
            assert_eq!(cookie_1.max_age(), Some(session_ttl));

            // Fire a request that doesn't touch the session state, check
            // that the session cookie is present and its expiry is set to the maximum we configured.
            let request = test::TestRequest::with_uri("/test/")
                .cookie(cookie_1)
                .to_request();
            let response = app.call(request).await.unwrap();
            let cookie_2 = response.get_cookie("id").expect("Cookie is set");
            assert_eq!(cookie_2.max_age(), Some(session_ttl));
        }

        pub(super) async fn expiration_is_refreshed_on_changes<F, Store>(
            store_builder: F,
            policy: CookieContentSecurity,
        ) where
            Store: SessionStore + 'static,
            F: Fn() -> Store + Clone + Send + 'static,
        {
            let session_ttl = time::Duration::seconds(60);
            let app = test::init_service(
                App::new()
                    .wrap(
                        SessionMiddleware::builder(store_builder(), key())
                            .cookie_content_security(policy)
                            .session_lifecycle(
                                PersistentSession::default().session_ttl(session_ttl),
                            )
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
            let cookie_1 = response.get_cookie("id").expect("Cookie is set");
            assert_eq!(cookie_1.max_age(), Some(session_ttl));

            let request = test::TestRequest::with_uri("/test/")
                .cookie(cookie_1.clone())
                .to_request();
            let response = app.call(request).await.unwrap();
            assert!(response.response().cookies().next().is_none());

            let request = test::TestRequest::get().cookie(cookie_1).to_request();
            let response = app.call(request).await.unwrap();
            let cookie_2 = response.get_cookie("id").expect("Cookie is set");
            assert_eq!(cookie_2.max_age(), Some(session_ttl));
        }

        pub(super) async fn guard<F, Store>(store_builder: F, policy: CookieContentSecurity)
        where
            Store: SessionStore + 'static,
            F: Fn() -> Store + Clone + Send + 'static,
        {
            let srv = actix_test::start(move || {
                App::new()
                    .wrap(
                        SessionMiddleware::builder(store_builder(), key())
                            .cookie_name("test-session".into())
                            .cookie_content_security(policy)
                            .session_lifecycle(
                                PersistentSession::default().session_ttl(time::Duration::days(7)),
                            )
                            .build(),
                    )
                    .wrap(middleware::Logger::default())
                    .service(resource("/").route(get().to(index)))
                    .service(resource("/do_something").route(post().to(do_something)))
                    .service(resource("/login").route(post().to(login)))
                    .service(resource("/logout").route(post().to(logout)))
                    .service(
                        web::scope("/protected")
                            .guard(guard::fn_guard(|g| {
                                g.get_session().get::<String>("user_id").unwrap().is_some()
                            }))
                            .service(resource("/count").route(get().to(count))),
                    )
            });

            // Step 1: GET without session info
            //   - response should be a unsuccessful status
            let req_1 = srv.get("/protected/count").send();
            let resp_1 = req_1.await.unwrap();
            assert!(!resp_1.status().is_success());

            // Step 2: POST to login
            //   - set-cookie actix-session will be in response  (session cookie #1)
            //   - updates session state: {"counter": 0, "user_id": "ferris"}
            let req_2 = srv.post("/login").send_json(&json!({"user_id": "ferris"}));
            let resp_2 = req_2.await.unwrap();
            let cookie_1 = resp_2
                .cookies()
                .unwrap()
                .clone()
                .into_iter()
                .find(|c| c.name() == "test-session")
                .unwrap();

            // Step 3: POST to do_something
            //   - adds new session state:  {"counter": 1, "user_id": "ferris" }
            //   - set-cookie actix-session should be in response (session cookie #2)
            //   - response should be: {"counter": 1, "user_id": None}
            let req_3 = srv.post("/do_something").cookie(cookie_1.clone()).send();
            let mut resp_3 = req_3.await.unwrap();
            let result_3 = resp_3.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_3,
                IndexResponse {
                    user_id: Some("ferris".into()),
                    counter: 1
                }
            );
            let cookie_2 = resp_3
                .cookies()
                .unwrap()
                .clone()
                .into_iter()
                .find(|c| c.name() == "test-session")
                .unwrap();

            // Step 4: GET using a existing user id
            //   - response should be: {"counter": 3, "user_id": "ferris"}
            let req_4 = srv.get("/protected/count").cookie(cookie_2.clone()).send();
            let mut resp_4 = req_4.await.unwrap();
            let result_4 = resp_4.json::<IndexResponse>().await.unwrap();
            assert_eq!(
                result_4,
                IndexResponse {
                    user_id: Some("ferris".into()),
                    counter: 1
                }
            );
        }

        pub(super) async fn complex_workflow<F, Store>(
            store_builder: F,
            is_invalidation_supported: bool,
            policy: CookieContentSecurity,
        ) where
            Store: SessionStore + 'static,
            F: Fn() -> Store + Clone + Send + 'static,
        {
            let session_ttl = time::Duration::days(7);
            let srv = actix_test::start(move || {
                App::new()
                    .wrap(
                        SessionMiddleware::builder(store_builder(), key())
                            .cookie_name("test-session".into())
                            .cookie_content_security(policy)
                            .session_lifecycle(
                                PersistentSession::default().session_ttl(session_ttl),
                            )
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
            assert_eq!(cookie_1.max_age(), Some(session_ttl));

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
            assert_eq!(cookie_2.max_age(), cookie_1.max_age());

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
            assert_eq!("/", cookie_3.path().unwrap());

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

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
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
            session.insert("counter", counter)?;

            Ok(HttpResponse::Ok().json(&IndexResponse { user_id, counter }))
        }

        async fn count(session: Session) -> Result<HttpResponse> {
            let user_id: Option<String> = session.get::<String>("user_id").unwrap();
            let counter: i32 = session.get::<i32>("counter").unwrap().unwrap();

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

            let body = if let Some(id) = id {
                session.purge();
                format!("Logged out: {id}")
            } else {
                "Could not log out anonymous user".to_owned()
            };

            Ok(HttpResponse::Ok().body(body))
        }

        trait ServiceResponseExt {
            fn get_cookie(&self, cookie_name: &str) -> Option<actix_web::cookie::Cookie<'_>>;
        }

        impl ServiceResponseExt for ServiceResponse {
            fn get_cookie(&self, cookie_name: &str) -> Option<actix_web::cookie::Cookie<'_>> {
                self.response().cookies().find(|c| c.name() == cookie_name)
            }
        }
    }
}
