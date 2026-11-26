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
//! # Ok::<_, russe::EncodeError>(())
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
//! # Ok::<_, russe::DecodeError>(())
//! ```
//!
//! # Client integrations
//!
//! Each integration provides an extension trait to read events from a response. The async clients
//! also provide a `Manager` to receive events through a channel.
//!
//! - `awc-3` for `awc` v3 extension trait and connection manager.
//! - `reqwest-0_12` for `reqwest` v0.12 extension trait and connection manager.
//! - `reqwest-0_13` for `reqwest` v0.13 extension trait and connection manager.
//! - `ureq-3` for `ureq` v3 blocking iterator.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "awc-3")]
#[path = "integrations/awc_3.rs"]
pub mod awc_3;
mod decoder;
mod encoder;
mod error;
mod event;
mod message;
#[cfg(feature = "reqwest-0_12")]
#[path = "integrations/reqwest_0_12.rs"]
pub mod reqwest_0_12;
#[cfg(feature = "reqwest-0_13")]
#[path = "integrations/reqwest_0_13.rs"]
pub mod reqwest_0_13;
#[cfg(feature = "ureq-3")]
#[path = "integrations/ureq_3.rs"]
pub mod ureq_3;

#[cfg(any(feature = "awc-3", feature = "reqwest-0_12", feature = "reqwest-0_13"))]
mod reconnect;
#[cfg(any(feature = "awc-3", feature = "reqwest-0_12", feature = "reqwest-0_13"))]
pub use self::reconnect::Reconnect;
pub use self::{
    decoder::Decoder,
    encoder::Encoder,
    error::{DecodeError, EncodeError},
    event::Event,
    message::Message,
};

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
