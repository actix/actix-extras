use std::{error::Error as StdError, fmt};

use actix_web::ResponseError;
use derive_more::Display;

#[derive(Debug, Display)]
pub(crate) enum InsertErrorKind {
    #[display(fmt = "{}", _0)]
    Json(serde_json::Error),
}

impl Into<actix_web::Error> for InsertErrorKind {
    fn into(self) -> actix_web::Error {
        match self {
            InsertErrorKind::Json(err) => err.into(),
        }
    }
}

/// Error returned by [`Session::insert`][crate::Session::insert]. Allows access to value that
/// failed to be inserted.
pub struct InsertError<T> {
    pub(crate) value: Option<T>,
    pub(crate) error: InsertErrorKind,
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

        dbg.field("error", &self.error).finish()
    }
}

impl<T> fmt::Display for InsertError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl<T: fmt::Debug> StdError for InsertError<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.error {
            InsertErrorKind::Json(err) => err,
        })
    }
}

impl<T> ResponseError for InsertError<T> {}
