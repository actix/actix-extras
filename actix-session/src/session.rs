use actix_web::{
    dev::{Extensions, ServiceRequest, ServiceResponse},
    Error, HttpMessage,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    mem,
    rc::Rc,
};

/// The primary interface to access and modify session state.
///
/// [`Session`] is an `actix-web` extractor - you can specify it as an input type for your request
/// handlers and it will be automatically extracted from the incoming request.
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
///
/// You can also retrieve a [`Session`] object from an `HttpRequest` or a `ServiceRequest` using
/// [`SessionExt`].
///
/// [`SessionExt`]: crate::SessionExt
pub struct Session(Rc<RefCell<SessionInner>>);

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

    /// Returns session status.
    pub fn status(&self) -> SessionStatus {
        Ref::map(self.0.borrow(), |inner| &inner.status).clone()
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

    pub(crate) fn get_session(extensions: &mut Extensions) -> Session {
        if let Some(s_impl) = extensions.get::<Rc<RefCell<SessionInner>>>() {
            return Session(Rc::clone(s_impl));
        }
        let inner = Rc::new(RefCell::new(SessionInner::default()));
        extensions.insert(inner.clone());
        Session(inner)
    }
}
