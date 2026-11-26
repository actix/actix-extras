//! High-performance Server-Sent Events (SSE) encoder and decoder.
//!
//! # Encoding (server-side)
//!
//! ```
//! # use indoc::indoc;
//! # use russe::{Event, Message};
//! let event = Event::Message(Message {
//!     data: "Hello World!\nHow are you, today?".into(),
//!     event: Some("chat".into()),
//!     retry: None,
//!     id: None,
//! });
//!
//! let expected = indoc! {"
//!     event: chat
//!     data: Hello World!
//!     data: How are you, today?
//!
//! "};
//!
//! assert_eq!(event.into_bytestring()?, expected);
//! # Ok::<_, russe::Error>(())
//! ```
//!
//! # Decoding (client-side)
//!
//! The client receives the server's encoded `chat` event in the response body.
//!
//! ```
//! # use bytes::BytesMut;
//! # use indoc::indoc;
//! # use russe::{Decoder, Event};
//! # use tokio_util::codec::Decoder as _;
//! let mut body = BytesMut::from(indoc! {"
//!     event: chat
//!     data: Hello World!
//!     data: How are you, today?
//!
//! "});
//!
//! let Event::Message(msg) = Decoder::default().decode(&mut body)?.unwrap() else {
//!     panic!("Expected a message event");
//! };
//!
//! assert_eq!(msg.event.unwrap(), "chat");
//! assert_eq!(msg.data, "Hello World!\nHow are you, today?");
//! # Ok::<_, russe::Error>(())
//! ```
//!
//! # Client integrations
//!
//! With Reqwest, use `ReqwestExt::sse_stream` to stream events from a response or `Manager` to send
//! a request and receive events through a channel. Enable the feature for your Reqwest version:
//!
//! - `reqwest-0_12` for Reqwest 0.12
//! - `reqwest-0_13` for Reqwest 0.13

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

pub(crate) const LF: u8 = b'\n';
pub(crate) const CR: u8 = b'\r';

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
