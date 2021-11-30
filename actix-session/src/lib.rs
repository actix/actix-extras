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
//! use actix_session::{Session, CookieSession};
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
//! #[actix_rt::main]
//! async fn main() -> std::io::Result<()> {
//!     HttpServer::new(
//!         || App::new()
//!             // create cookie based session middleware
//!             .wrap(CookieSession::signed(&[0; 32]).secure(false))
//!             .default_service(web::to(|| HttpResponse::Ok())))
//!         .bind(("127.0.0.1", 8080))?
//!         .run()
//!         .await
//! }
//! ```

#![deny(rust_2018_idioms, nonstandard_style)]
#![warn(missing_docs)]

#[cfg(feature = "cookie-session")]
pub use storage::CookieSession;
#[cfg(feature = "redis-actor-session")]
pub use storage::RedisActorSession;

pub use extractors::UserSession;
pub use session::{Session, SessionStatus};

mod extractors;
mod session;
mod storage;

#[cfg(test)]
mod tests {
    use actix_web::{test, HttpResponse};

    use super::*;

    #[actix_web::test]
    async fn session() {
        let mut req = test::TestRequest::default().to_srv_request();

        Session::set_session(
            &mut req,
            vec![("key".to_string(), serde_json::to_string("value").unwrap())],
        );
        let session = req.get_session();
        let res = session.get::<String>("key").unwrap();
        assert_eq!(res, Some("value".to_string()));

        session.insert("key2", "value2").unwrap();
        session.remove("key");

        let mut res = req.into_response(HttpResponse::Ok().finish());
        let (_status, state) = Session::get_changes(&mut res);
        let changes: Vec<_> = state.collect();
        assert_eq!(changes, [("key2".to_string(), "\"value2\"".to_string())]);
    }

    #[actix_web::test]
    async fn get_session() {
        let mut req = test::TestRequest::default().to_srv_request();

        Session::set_session(
            &mut req,
            vec![("key".to_string(), serde_json::to_string(&true).unwrap())],
        );

        let session = req.get_session();
        let res = session.get("key").unwrap();
        assert_eq!(res, Some(true));
    }

    #[actix_web::test]
    async fn get_session_from_request_head() {
        let mut req = test::TestRequest::default().to_srv_request();

        Session::set_session(
            &mut req,
            vec![("key".to_string(), serde_json::to_string(&10).unwrap())],
        );

        let session = req.head_mut().get_session();
        let res = session.get::<u32>("key").unwrap();
        assert_eq!(res, Some(10));
    }

    #[actix_web::test]
    async fn purge_session() {
        let req = test::TestRequest::default().to_srv_request();
        let session = req.get_session();
        assert_eq!(session.status(), SessionStatus::Unchanged);
        session.purge();
        assert_eq!(session.status(), SessionStatus::Purged);
    }

    #[actix_web::test]
    async fn renew_session() {
        let req = test::TestRequest::default().to_srv_request();
        let session = req.get_session();
        assert_eq!(session.status(), SessionStatus::Unchanged);
        session.renew();
        assert_eq!(session.status(), SessionStatus::Renewed);
    }

    #[actix_web::test]
    async fn session_entries() {
        let mut req = test::TestRequest::default().to_srv_request();
        Session::set_session(
            &mut req,
            vec![
                ("test_str".to_string(), "val".into()),
                ("test_num".to_string(), "1".into()),
            ],
        );

        let session = req.get_session();
        let map = session.entries();
        map.contains_key("test_str");
        map.contains_key("test_num");
    }
}
