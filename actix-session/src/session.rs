use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    mem,
    rc::Rc,
};

use actix_utils::future::{ready, Ready};
use actix_web::{
    dev::{Extensions, Payload, ServiceRequest, ServiceResponse},
    error::Error,
    FromRequest, HttpMessage, HttpRequest,
};
use serde::{de::DeserializeOwned, Serialize};

/// The primary interface to access and modify session state.
///
/// [`Session`] is an [extractor](#impl-FromRequest)—you can specify it as an input type for your
/// request handlers and it will be automatically extracted from the incoming request.
///
/// ```
/// use actix_session::Session;
///
/// async fn index(session: Session) -> actix_web::Result<&'static str> {
///     // access session data
///     if let Some(count) = session.get::<i32>("counter")? {
///         session.insert("counter", count + 1)?;
///     } else {
///         session.insert("counter", 1)?;
///     }
///
///     Ok("Welcome!")
/// }
/// # actix_web::web::to(index);
/// ```
///
/// You can also retrieve a [`Session`] object from an `HttpRequest` or a `ServiceRequest` using
/// [`SessionExt`].
///
/// [`SessionExt`]: crate::SessionExt
pub struct Session(Rc<RefCell<SessionInner>>);

/// Status of a [`Session`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    /// Session state has been updated - the changes will have to be persisted to the backend.
    Changed,

    /// The session has been flagged for deletion - the session cookie will be removed from
    /// the client and the session state will be deleted from the session store.
    ///
    /// Most operations on the session after it has been marked for deletion will have no effect.
    Purged,

    /// The session has been flagged for renewal.
    ///
    /// The session key will be regenerated and the time-to-live of the session state will be
    /// extended.
    Renewed,

    /// The session state has not been modified since its creation/retrieval.
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
    ///
    /// It returns an error if it fails to deserialize as `T` the JSON value associated with `key`.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, serde_json::Error> {
        if let Some(val_str) = self.0.borrow().state.get(key) {
            Ok(Some(serde_json::from_str(val_str)?))
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

    /// Returns session status.
    pub fn status(&self) -> SessionStatus {
        Ref::map(self.0.borrow(), |inner| &inner.status).clone()
    }

    /// Inserts a key-value pair into the session.
    ///
    /// Any serializable value can be used and will be encoded as JSON in session data, hence why
    /// only a reference to the value is taken.
    ///
    /// It returns an error if it fails to serialize `value` to JSON.
    pub fn insert(
        &self,
        key: impl Into<String>,
        value: impl Serialize,
    ) -> Result<(), serde_json::Error> {
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
    /// Returns None if key was not present in session. Returns `T` if deserialization succeeds,
    /// otherwise returns un-deserialized JSON string.
    pub fn remove_as<T: DeserializeOwned>(&self, key: &str) -> Option<Result<T, String>> {
        self.remove(key)
            .map(|val_str| match serde_json::from_str(&val_str) {
                Ok(val) => Ok(val),
                Err(_err) => {
                    tracing::debug!(
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
    pub(crate) fn set_session(
        req: &mut ServiceRequest,
        data: impl IntoIterator<Item = (String, String)>,
    ) {
        let session = Session::get_session(&mut *req.extensions_mut());
        let mut inner = session.0.borrow_mut();
        inner.state.extend(data);
    }

    /// Returns session status and iterator of key-value pairs of changes.
    ///
    /// This is a destructive operation - the session state is removed from the request extensions typemap,
    /// leaving behind a new empty map. It should only be used when the session is being finalised (i.e.
    /// in `SessionMiddleware`).
    pub(crate) fn get_changes<B>(
        res: &mut ServiceResponse<B>,
    ) -> (SessionStatus, HashMap<String, String>) {
        if let Some(s_impl) = res
            .request()
            .extensions()
            .get::<Rc<RefCell<SessionInner>>>()
        {
            let state = mem::take(&mut s_impl.borrow_mut().state);
            (s_impl.borrow().status.clone(), state)
        } else {
            (SessionStatus::Unchanged, HashMap::new())
        }
    }

    pub(crate) fn get_session(extensions: &mut Extensions) -> Session {
        if let Some(s_impl) = extensions.get::<Rc<RefCell<SessionInner>>>() {
            return Session(Rc::clone(s_impl));
        }

        let inner = Rc::new(RefCell::new(SessionInner::default()));
        extensions.insert(inner.clone());

        Session(inner)
    }
}

/// Extractor implementation for [`Session`]s.
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

    #[inline]
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ready(Ok(Session::get_session(&mut *req.extensions_mut())))
    }
}
