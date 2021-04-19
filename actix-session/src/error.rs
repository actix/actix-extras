//! Implementation of errors generated when interacting with a [Session][crate::Session]. While
//! operations on [Session][crate::Session] can fail with [actix_web::Error], these errors can be
//! downcast to [InsertError].
//!
//! # Examples
//!
//! ```
//!     use actix_session::Session;
//!     use actix_session::error::InsertError;
//!     use actix_web::ResponseError;
//!     use actix_web::test;
//!     use actix_session::UserSession;
//!
//!     let mut req = test::TestRequest::default().to_srv_request();
//!
//!     Session::set_session(
//!         &mut req,
//!         vec![("key".to_string(), r#"{"key":value}"#.to_string())],
//!     );
//!
//!     let session = req.get_session();
//!     let actix_err = session.get::<String>("key").unwrap_err();
//!     let downcast: &InsertError<()> = actix_err.as_error().unwrap();
//!     // If an insert had failed, the value which wasn't insertable would be retrievable here
//!     println!("{:?}", downcast);
//! ```

use std::{error::Error as StdError, fmt};

use actix_web::ResponseError;
use derive_more::Display;

/// Error returned by [`Session::get`][crate::Session::get]
#[derive(Debug, Display)]
pub(crate) enum InsertErrorKind {
    /// Is returned in case of a json serilization error
    #[display(fmt = "{}", _0)]
    Json(serde_json::Error),
}

impl From<serde_json::Error> for InsertErrorKind {
    fn from(e: serde_json::Error) -> Self {
        InsertErrorKind::Json(e)
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

impl<T> From<InsertErrorKind> for InsertError<T> {
    fn from(error: InsertErrorKind) -> Self {
        InsertError::<T> { value: None, error }
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

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_value_once() {
        InsertError {
            value: Some("This is text"),
            error: InsertErrorKind::Json(serde_json::Error{
                err: (),

            }),
        }
    }
} */
