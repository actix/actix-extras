use core::{error::Error as StdError, str};
use std::io;

/// SSE request, encoding, or decoding error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An event ID or event name contains a forbidden character.
    InvalidFieldValue {
        /// Field name: `id` or `event`.
        field: &'static str,

        /// Forbidden character.
        character: char,

        /// Zero-based byte offset of the character in the original value.
        offset: usize,
    },

    /// Stream contained invalid UTF-8.
    InvalidUtf8 {
        /// Source error.
        source: str::Utf8Error,
    },

    /// Initial HTTP request failed.
    Http {
        /// Boxed to support multiple HTTP clients.
        source: Box<dyn StdError + Send + Sync>,
    },

    /// I/O error.
    Io {
        /// Source error.
        source: io::Error,
    },
}

impl Error {
    pub(crate) fn invalid_field_value(field: &'static str, value: &str, offset: usize) -> Self {
        Self::InvalidFieldValue {
            field,
            character: char::from(value.as_bytes()[offset]),
            offset,
        }
    }
}

impl_more::impl_display_enum! {
    Error:
    InvalidFieldValue { field, character, offset } => "Invalid SSE {field}: {character:?} at byte {offset}",
    InvalidUtf8 { .. } => "Stream contained invalid UTF-8",
    Http { .. } => "HTTP request failed",
    Io { .. } => "I/O error",
}

impl_more::impl_error_enum! {
    Error:
    InvalidUtf8 { source } => source,
    Http { source } => source.as_ref(),
    Io { source } => source,
}

impl_more::impl_enum_from!(str::Utf8Error => Error::InvalidUtf8 { source });
impl_more::impl_enum_from!(io::Error => Error::Io { source });
