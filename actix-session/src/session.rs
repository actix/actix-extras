use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    error::Error as StdError,
    mem,
    rc::Rc,
};

use actix_utils::future::{ready, Ready};
use actix_web::{
    body::BoxBody,
    dev::{Extensions, Payload, ServiceRequest, ServiceResponse},
    error::Error,
    FromRequest, HttpMessage, HttpRequest, HttpResponse, ResponseError,
};
use anyhow::Context;
use derive_more::derive::{Display, From};
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
///     // or use the shorthand
///     session.update_or("counter", 1, |count: i32| count + 1);
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
#[derive(Clone)]
pub struct Session(Rc<RefCell<SessionInner>>);

/// Status of a [`Session`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    #[default]
    Unchanged,
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
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, SessionGetError> {
        if let Some(val_str) = self.0.borrow().state.get(key) {
            Ok(Some(
                serde_json::from_str(val_str)
                    .with_context(|| {
                        format!(
                            "Failed to deserialize the JSON-encoded session data attached to key \
                            `{}` as a `{}` type",
                            key,
                            std::any::type_name::<T>()
                        )
                    })
                    .map_err(SessionGetError)?,
            ))
        } else {
            Ok(None)
        }
    }

    /// Returns `true` if the session contains a value for the specified `key`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.borrow().state.contains_key(key)
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
    /// # Errors
    ///
    /// Returns an error if JSON serialization of `value` fails.
    pub fn insert<T: Serialize>(
        &self,
        key: impl Into<String>,
        value: T,
    ) -> Result<(), SessionInsertError> {
        let mut inner = self.0.borrow_mut();

        if inner.status != SessionStatus::Purged {
            if inner.status != SessionStatus::Renewed {
                inner.status = SessionStatus::Changed;
            }

            let key = key.into();
            let val = serde_json::to_string(&value)
                .with_context(|| {
                    format!(
                        "Failed to serialize the provided `{}` type instance as JSON in order to \
                        attach as session data to the `{key}` key",
                        std::any::type_name::<T>(),
                    )
                })
                .map_err(SessionInsertError)?;

            inner.state.insert(key, val);
        }

        Ok(())
    }

    /// Updates a key-value pair into the session.
    ///
    /// If the key exists then update it to the new value and place it back in. If the key does not
    /// exist it will not be updated.
    ///
    /// Any serializable value can be used and will be encoded as JSON in the session data, hence
    /// why only a reference to the value is taken.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization of the value fails.
    pub fn update<T: Serialize + DeserializeOwned, F>(
        &self,
        key: impl Into<String>,
        updater: F,
    ) -> Result<(), SessionUpdateError>
    where
        F: FnOnce(T) -> T,
    {
        let mut inner = self.0.borrow_mut();
        let key_str = key.into();

        if let Some(val_str) = inner.state.get(&key_str) {
            let value = serde_json::from_str(val_str)
                .with_context(|| {
                    format!(
                        "Failed to deserialize the JSON-encoded session data attached to key \
                        `{key_str}` as a `{}` type",
                        std::any::type_name::<T>()
                    )
                })
                .map_err(SessionUpdateError)?;

            let val = serde_json::to_string(&updater(value))
                .with_context(|| {
                    format!(
                        "Failed to serialize the provided `{}` type instance as JSON in order to \
                        attach as session data to the `{key_str}` key",
                        std::any::type_name::<T>(),
                    )
                })
                .map_err(SessionUpdateError)?;

            inner.state.insert(key_str, val);
        }

        Ok(())
    }

    /// Updates a key-value pair into the session, or inserts a default value.
    ///
    /// If the key exists then update it to the new value and place it back in. If the key does not
    /// exist the default value will be inserted instead.
    ///
    /// Any serializable value can be used and will be encoded as JSON in session data, hence why
    /// only a reference to the value is taken.
    ///
    /// # Errors
    ///
    /// Returns error if JSON serialization of a value fails.
    pub fn update_or<T: Serialize + DeserializeOwned, F>(
        &self,
        key: &str,
        default_value: T,
        updater: F,
    ) -> Result<(), SessionUpdateError>
    where
        F: FnOnce(T) -> T,
    {
        if self.contains_key(key) {
            self.update(key, updater)
        } else {
            self.insert(key, default_value)
                .map_err(|err| SessionUpdateError(err.into()))
        }
    }

    /// Remove value from the session.
    ///
    /// If present, the JSON encoded value is returned.
    pub fn remove(&self, key: &str) -> Option<String> {
        let mut inner = self.0.borrow_mut();

        if inner.status != SessionStatus::Purged {
            if inner.status != SessionStatus::Renewed {
                inner.status = SessionStatus::Changed;
            }
            return inner.state.remove(key);
        }

        None
    }

    /// Remove value from the session and deserialize.
    ///
    /// Returns `None` if key was not present in session. Returns `T` if deserialization succeeds,
    /// otherwise returns un-deserialized JSON string.
    pub fn remove_as<T: DeserializeOwned>(&self, key: &str) -> Option<Result<T, String>> {
        self.remove(key)
            .map(|val_str| match serde_json::from_str(&val_str) {
                Ok(val) => Ok(val),
                Err(_err) => {
                    tracing::debug!(
                        "Removed value (key: {}) could not be deserialized as {}",
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
            if inner.status != SessionStatus::Renewed {
                inner.status = SessionStatus::Changed;
            }
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
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub(crate) fn set_session(
        req: &mut ServiceRequest,
        data: impl IntoIterator<Item = (String, String)>,
    ) {
        let session = Session::get_session(&mut req.extensions_mut());
        let mut inner = session.0.borrow_mut();
        inner.state.extend(data);
    }

    /// Returns session status and iterator of key-value pairs of changes.
    ///
    /// This is a destructive operation - the session state is removed from the request extensions
    /// typemap, leaving behind a new empty map. It should only be used when the session is being
    /// finalised (i.e. in `SessionMiddleware`).
    #[allow(clippy::needless_pass_by_ref_mut)]
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
        ready(Ok(Session::get_session(&mut req.extensions_mut())))
    }
}

/// Error returned by [`Session::get`].
#[derive(Debug, Display, From)]
#[display("{_0}")]
pub struct SessionGetError(anyhow::Error);

impl StdError for SessionGetError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.0.as_ref())
    }
}

impl ResponseError for SessionGetError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::new(self.status_code())
    }
}

/// Error returned by [`Session::insert`].
#[derive(Debug, Display, From)]
#[display("{_0}")]
pub struct SessionInsertError(anyhow::Error);

impl StdError for SessionInsertError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.0.as_ref())
    }
}

impl ResponseError for SessionInsertError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::new(self.status_code())
    }
}

/// Error returned by [`Session::update`].
#[derive(Debug, Display, From)]
#[display("{_0}")]
pub struct SessionUpdateError(anyhow::Error);

impl StdError for SessionUpdateError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.0.as_ref())
    }
}

impl ResponseError for SessionUpdateError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::new(self.status_code())
    }
}
