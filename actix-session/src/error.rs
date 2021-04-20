//! Implementation of errors generated when interacting with a [Session][crate::Session].

use std::{error::Error as StdError, fmt};

use actix_web::ResponseError;
use derive_more::Display;

/// Error returned by [`Session::get`][crate::Session::get]
#[derive(Debug, Display)]
pub(crate) enum ErrorSource {
    /// Is returned in case of a json serilization error
    #[display(fmt = "{}", _0)]
    Json(serde_json::Error),
}

impl From<serde_json::Error> for ErrorSource {
    fn from(e: serde_json::Error) -> Self {
        ErrorSource::Json(e)
    }
}

/// Error returned by [`Session::insert`][crate::Session::insert]. Allows access to value that
/// failed to be inserted.
pub struct InsertError<T> {
    pub(crate) value: Option<T>,
    pub(crate) source: ErrorSource,
}

impl<T> InsertError<T> {
    /// Takes value out of error.
    ///
    /// # Panics
    /// Panics if called more than once.
    pub fn take_value(&mut self) -> T {
        self.value
            .take()
            .expect("take_value should only be called once")
    }
}

impl<T> fmt::Debug for InsertError<T> {
    fn fmt<'a>(&'a self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("SessionInsertError");

        match &self.value {
            Some(_) => dbg.field("value", &"Some([value])" as _),
            None => dbg.field("value", &None::<()> as _),
        };

        dbg.field("error", &self.source).finish()
    }
}

impl<T> fmt::Display for InsertError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.source, f)
    }
}

impl<T: fmt::Debug> StdError for InsertError<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.source {
            ErrorSource::Json(err) => err,
        })
    }
}

impl<T> ResponseError for InsertError<T> {}

/// The error type returned by [`Session::get`][crate::Session::get]
pub struct GetError {
    pub(crate) source: ErrorSource,
}

impl fmt::Display for GetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.source, f)
    }
}

impl fmt::Debug for GetError {
    fn fmt<'a>(&'a self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("SessionGetError");
        dbg.field("error", &self.source).finish()
    }
}

impl StdError for GetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.source {
            ErrorSource::Json(err) => err,
        })
    }
}

impl ResponseError for GetError {}
