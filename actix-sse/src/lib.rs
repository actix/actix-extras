//! Semantic server-sent events (SSE) responder
//!
//! # Examples
//! ```no_run
//! use std::{convert::Infallible, time::Duration};
//!
//! use actix_web::{Responder, get};
//! use tokio_stream::wrappers::ReceiverStream;
//!
//! #[get("/from-channel")]
//! async fn from_channel() -> impl Responder {
//!     let (tx, rx) = tokio::sync::mpsc::channel(10);
//!
//!     // note: sender will typically be spawned or handed off somewhere else
//!     let _ = tx.send(actix_sse::Event::Comment("my comment".into())).await;
//!     let _ = tx
//!         .send(actix_sse::Data::new("my data").event("chat_msg").into())
//!         .await;
//!
//!     let event_stream = ReceiverStream::new(rx);
//!     actix_sse::Sse::from_infallible_stream(event_stream).with_retry_duration(Duration::from_secs(10))
//! }
//!
//! #[get("/from-stream")]
//! async fn from_stream() -> impl Responder {
//!     let event_stream = futures_util::stream::iter([Ok::<_, Infallible>(actix_sse::Event::Data(
//!         actix_sse::Data::new("foo"),
//!     ))]);
//!
//!     actix_sse::Sse::from_stream(event_stream).with_keep_alive(Duration::from_secs(5))
//! }
//! ```

use self::stream::InfallibleStream;
pub use self::{data::Data, event::Event, sse::Sse};

mod data;
mod event;
mod sse;
mod stream;
