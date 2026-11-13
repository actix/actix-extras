//! Server-Sent Events (SSE) encoder and decoder.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod decoder;
mod encoder;
mod error;
mod event;
mod message;
#[cfg(feature = "reqwest-0_12")]
pub mod reqwest_0_12;
#[cfg(feature = "reqwest-0_13")]
pub mod reqwest_0_13;

pub use self::{decoder::Decoder, encoder::Encoder, error::Error, event::Event, message::Message};

/// A specialized `Result` type for `russe` operations.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) const NEWLINE: u8 = b'\n';
pub(crate) const SSE_DELIMITER: &[u8] = b"\n\n";

/// Media (MIME) type for SSE (`text/event-stream`).
#[cfg(feature = "mime")]
pub const MEDIA_TYPE: mime::Mime = mime::TEXT_EVENT_STREAM;

/// Media (MIME) type for SSE (`text/event-stream`).
pub const MEDIA_TYPE_STR: &str = "text/event-stream";

#[cfg(test)]
mod tests {
    /// Asserts that `Option<T>` argument is `None`.
    #[macro_export]
    macro_rules! assert_none {
        ($exp:expr) => {{
            let exp = $exp;
            assert!(exp.is_none(), "Expected None; got: {exp:?}");
        }};
    }
}
