//! Semantic server-sent events (SSE) responder.
//!
//! Message data and comments can contain multiple lines. CRLF and CR line endings are converted
//! to LF. An ID with NUL, CR, or LF, or an event name with CR or LF, causes the response body to
//! return an encoding error.
//!
//! # Examples
//! ```no_run
//! use std::{convert::Infallible, time::Duration};
//!
//! use actix_web::{Responder, get};
//! use actix_sse as sse;
//!
//! #[get("/from-channel")]
//! async fn from_channel() -> impl Responder {
//!     let (tx, rx) = tokio::sync::mpsc::channel(10);
//!
//!     // note: sender will typically be spawned or handed off somewhere else
//!     let _ = tx.send(sse::Event::Comment("my comment".into())).await;
//!     let _ = tx
//!         .send(sse::Data::new("my data").event("chat_msg").into())
//!         .await;
//!
//!     sse::Sse::from_infallible_receiver(rx).with_retry_duration(Duration::from_secs(10))
//! }
//!
//! #[get("/from-stream")]
//! async fn from_stream() -> impl Responder {
//!     let event_stream = futures_util::stream::iter([Ok::<_, Infallible>(sse::Event::Data(
//!         sse::Data::new("foo"),
//!     ))]);
//!
//!     sse::Sse::from_stream(event_stream).with_keep_alive(Duration::from_secs(5))
//! }
//! ```

mod data;
mod event;
mod sse;

pub use self::{data::Data, event::Event, sse::Sse};

type BoxError = Box<dyn std::error::Error>;
