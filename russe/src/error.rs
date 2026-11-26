use std::{io, str};

/// Error returned when encoding an SSE stream.
#[derive(Debug)]
#[non_exhaustive]
pub enum EncodeError {
    /// An event ID or event name contains a forbidden character.
    InvalidFieldValue {
        /// Field name: `id` or `event`.
        field: &'static str,

        /// Forbidden character.
        character: char,

        /// Zero-based byte offset of the character in the original value.
        offset: usize,
    },

    /// I/O error.
    Io {
        /// Source error.
        source: io::Error,
    },
}

impl EncodeError {
    pub(crate) fn invalid_field_value(field: &'static str, value: &str, offset: usize) -> Self {
        Self::InvalidFieldValue {
            field,
            character: char::from(value.as_bytes()[offset]),
            offset,
        }
    }
}

impl_more::impl_display_enum! {
    EncodeError:
    InvalidFieldValue { field, character, offset } => "Invalid SSE {field}: {character:?} at byte {offset}",
    Io { .. } => "I/O error",
}

impl_more::impl_error_enum! {
    EncodeError:
    Io { source } => source,
}

impl_more::impl_enum_from!(io::Error => EncodeError::Io { source });

/// Error returned when decoding an SSE stream.
#[derive(Debug)]
#[non_exhaustive]
pub enum DecodeError {
    /// Stream contained invalid UTF-8.
    InvalidUtf8 {
        /// Source error.
        source: str::Utf8Error,
    },

    /// I/O error.
    Io {
        /// Source error.
        source: io::Error,
    },
}

impl_more::impl_display_enum! {
    DecodeError:
    InvalidUtf8 { .. } => "Stream contained invalid UTF-8",
    Io { .. } => "I/O error",
}

impl_more::impl_error_enum! {
    DecodeError:
    InvalidUtf8 { source } => source,
    Io { source } => source,
}

impl_more::impl_enum_from!(io::Error => DecodeError::Io { source });
