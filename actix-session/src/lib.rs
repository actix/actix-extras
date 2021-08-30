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

use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    mem,
    rc::Rc,
};

use actix_web::{
    dev::{Extensions, Payload, RequestHead, ServiceRequest, ServiceResponse},
    Error, FromRequest, HttpMessage, HttpRequest,
};
use futures_util::future::{ok, Ready};
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "cookie-session")]
mod cookie;
#[cfg(feature = "cookie-session")]
pub use self::cookie::CookieSession;

/// The high-level interface you use to modify session data.
///
/// Session object is obtained with [`UserSession::get_session`]. The [`UserSession`] trait is
/// implemented for `HttpRequest`, `ServiceRequest`, and `RequestHead`.
///
/// ```
/// use actix_session::Session;
/// use actix_web::Result;
///
/// async fn index(session: Session) -> Result<&'static str> {
///     // access session data
///     if let Some(count) = session.get::<i32>("counter")? {
///         session.insert("counter", count + 1)?;
///     } else {
///         session.insert("counter", 1)?;
///     }
///
///     Ok("Welcome!")
/// }
/// ```
pub struct Session(Rc<RefCell<SessionInner>>);

/// Extraction of a [`Session`] object.
pub trait UserSession {
    /// Extract the [`Session`] object
    fn get_session(&self) -> Session;
}

impl UserSession for HttpRequest {
    fn get_session(&self) -> Session {
        Session::get_session(&mut *self.extensions_mut())
    }
}

impl UserSession for ServiceRequest {
    fn get_session(&self) -> Session {
        Session::get_session(&mut *self.extensions_mut())
    }
}

impl UserSession for RequestHead {
    fn get_session(&self) -> Session {
        Session::get_session(&mut *self.extensions_mut())
    }
}

/// Status of a [`Session`].
#[derive(PartialEq, Clone, Debug)]
pub enum SessionStatus {
    /// Session has been updated and requires a new persist operation.
    Changed,

    /// Session is flagged for deletion and should be removed from client and server.
    ///
    /// Most operations on the session after purge flag is set should have no effect.
    Purged,

    /// Session is flagged for refresh.
    ///
    /// For example, when using a backend that has a TTL (time-to-live) expiry on the session entry,
    /// the session will be refreshed even if no data inside it has changed. The client may also
    /// be notified of the refresh.
    Renewed,

    /// Session is unchanged from when last seen (if exists).
    ///
    /// This state also captures new (previously unissued) sessions such as a user's first
    /// site visit.
    Unchanged,
}

impl Default for SessionStatus {
    fn default() -> SessionStatus {
        SessionStatus::Unchanged
    }
}

#[derive(Default)]
struct SessionInner {
    state: HashMap<String, String>,
    status: SessionStatus,
}

impl Session {
    /// Get a `value` from the session.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        if let Some(s) = self.0.borrow().state.get(key) {
            Ok(Some(serde_json::from_str(s)?))
        } else {
            Ok(None)
        }
    }

    /// Get all raw key-value data from the session.
    ///
    /// Note that values are JSON encoded.
    pub fn entries(&self) -> Ref<'_, HashMap<String, String>> {
        Ref::map(self.0.borrow(), |inner| &inner.state)
    }

    /// Inserts a key-value pair into the session.
    ///
    /// Any serializable value can be used and will be encoded as JSON in session data, hence why
    /// only a reference to the value is taken.
    pub fn insert(&self, key: impl Into<String>, value: impl Serialize) -> Result<(), Error> {
        let mut inner = self.0.borrow_mut();

        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Changed;
            let val = serde_json::to_string(&value)?;
            inner.state.insert(key.into(), val);
        }

        Ok(())
    }

    /// Remove value from the session.
    ///
    /// If present, the JSON encoded value is returned.
    pub fn remove(&self, key: &str) -> Option<String> {
        let mut inner = self.0.borrow_mut();

        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Changed;
            return inner.state.remove(key);
        }

        None
    }

    /// Remove value from the session and deserialize.
    ///
    /// Returns None if key was not present in session. Returns T if deserialization succeeds,
    /// otherwise returns un-deserialized JSON string.
    pub fn remove_as<T: DeserializeOwned>(&self, key: &str) -> Option<Result<T, String>> {
        self.remove(key)
            .map(|val_str| match serde_json::from_str(&val_str) {
                Ok(val) => Ok(val),
                Err(_err) => {
                    log::debug!(
                        "removed value (key: {}) could not be deserialized as {}",
                        key,
                        std::any::type_name::<T>()
                    );
                    Err(val_str)
                }
            })
    }

    /// Clear the session.
    pub fn clear(&self) {
        let mut inner = self.0.borrow_mut();

        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Changed;
            inner.state.clear()
        }
    }

    /// Removes session both client and server side.
    pub fn purge(&self) {
        let mut inner = self.0.borrow_mut();
        inner.status = SessionStatus::Purged;
        inner.state.clear();
    }

    /// Renews the session key, assigning existing session state to new key.
    pub fn renew(&self) {
        let mut inner = self.0.borrow_mut();

        if inner.status != SessionStatus::Purged {
            inner.status = SessionStatus::Renewed;
        }
    }

    /// Adds the given key-value pairs to the session on the request.
    ///
    /// Values that match keys already existing on the session will be overwritten. Values should
    /// already be JSON serialized.
    ///
    /// # Examples
    /// ```
    /// # use actix_session::Session;
    /// # use actix_web::test;
    /// let mut req = test::TestRequest::default().to_srv_request();
    ///
    /// Session::set_session(
    ///     &mut req,
    ///     vec![("counter".to_string(), serde_json::to_string(&0).unwrap())],
    /// );
    /// ```
    pub fn set_session(req: &mut ServiceRequest, data: impl IntoIterator<Item = (String, String)>) {
        let session = Session::get_session(&mut *req.extensions_mut());
        let mut inner = session.0.borrow_mut();
        inner.state.extend(data);
    }

    /// Returns session status and iterator of key-value pairs of changes.
    pub fn get_changes<B>(
        res: &mut ServiceResponse<B>,
    ) -> (SessionStatus, impl Iterator<Item = (String, String)>) {
        if let Some(s_impl) = res
            .request()
            .extensions()
            .get::<Rc<RefCell<SessionInner>>>()
        {
            let state = mem::take(&mut s_impl.borrow_mut().state);
            (s_impl.borrow().status.clone(), state.into_iter())
        } else {
            (SessionStatus::Unchanged, HashMap::new().into_iter())
        }
    }

    fn get_session(extensions: &mut Extensions) -> Session {
        if let Some(s_impl) = extensions.get::<Rc<RefCell<SessionInner>>>() {
            return Session(Rc::clone(s_impl));
        }
        let inner = Rc::new(RefCell::new(SessionInner::default()));
        extensions.insert(inner.clone());
        Session(inner)
    }
}

/// Extractor implementation for Session type.
///
/// # Examples
/// ```
/// # use actix_web::*;
/// use actix_session::Session;
///
/// #[get("/")]
/// async fn index(session: Session) -> Result<impl Responder> {
///     // access session data
///     if let Some(count) = session.get::<i32>("counter")? {
///         session.insert("counter", count + 1)?;
///     } else {
///         session.insert("counter", 1)?;
///     }
///
///     let count = session.get::<i32>("counter")?.unwrap();
///     Ok(format!("Counter: {}", count))
/// }
/// ```
impl FromRequest for Session {
    type Error = Error;
    type Future = Ready<Result<Session, Error>>;
    type Config = ();

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ok(Session::get_session(&mut *req.extensions_mut()))
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{test, HttpResponse};

    use super::*;

    #[test]
    fn session() {
        let mut req = test::TestRequest::default().to_srv_request();

        Session::set_session(
            &mut req,
            vec![("key".to_string(), serde_json::to_string("value").unwrap())],
        );
        let session = Session::get_session(&mut *req.extensions_mut());
        let res = session.get::<String>("key").unwrap();
        assert_eq!(res, Some("value".to_string()));

        session.insert("key2", "value2").unwrap();
        session.remove("key");

        let mut res = req.into_response(HttpResponse::Ok().finish());
        let (_status, state) = Session::get_changes(&mut res);
        let changes: Vec<_> = state.collect();
        assert_eq!(changes, [("key2".to_string(), "\"value2\"".to_string())]);
    }

    #[test]
    fn get_session() {
        let mut req = test::TestRequest::default().to_srv_request();

        Session::set_session(
            &mut req,
            vec![("key".to_string(), serde_json::to_string(&true).unwrap())],
        );

        let session = req.get_session();
        let res = session.get("key").unwrap();
        assert_eq!(res, Some(true));
    }

    #[test]
    fn get_session_from_request_head() {
        let mut req = test::TestRequest::default().to_srv_request();

        Session::set_session(
            &mut req,
            vec![("key".to_string(), serde_json::to_string(&10).unwrap())],
        );

        let session = req.head_mut().get_session();
        let res = session.get::<u32>("key").unwrap();
        assert_eq!(res, Some(10));
    }

    #[test]
    fn purge_session() {
        let req = test::TestRequest::default().to_srv_request();
        let session = Session::get_session(&mut *req.extensions_mut());
        assert_eq!(session.0.borrow().status, SessionStatus::Unchanged);
        session.purge();
        assert_eq!(session.0.borrow().status, SessionStatus::Purged);
    }

    #[test]
    fn renew_session() {
        let req = test::TestRequest::default().to_srv_request();
        let session = Session::get_session(&mut *req.extensions_mut());
        assert_eq!(session.0.borrow().status, SessionStatus::Unchanged);
        session.renew();
        assert_eq!(session.0.borrow().status, SessionStatus::Renewed);
    }

    #[test]
    fn session_entries() {
        let session = Session(Rc::new(RefCell::new(SessionInner::default())));
        session.insert("test_str", "val").unwrap();
        session.insert("test_num", 1).unwrap();

        let map = session.entries();
        map.contains_key("test_str");
        map.contains_key("test_num");
    }
}
