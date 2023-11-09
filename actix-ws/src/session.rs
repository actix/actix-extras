use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use actix_http::ws::{CloseReason, Message};
use actix_web::web::Bytes;
use bytestring::ByteString;
use tokio::sync::mpsc::Sender;

/// A handle into the websocket session.
///
/// This type can be used to send messages into the websocket.
#[derive(Clone)]
pub struct Session {
    inner: Option<Sender<Message>>,
    closed: Arc<AtomicBool>,
}

/// The error representing a closed websocket session
#[derive(Debug)]
pub struct Closed;

impl std::fmt::Display for Closed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Session is closed")
    }
}

impl std::error::Error for Closed {}

impl Session {
    pub(super) fn new(inner: Sender<Message>) -> Self {
        Session {
            inner: Some(inner),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn pre_check(&mut self) {
        if self.closed.load(Ordering::Relaxed) {
            self.inner.take();
        }
    }

    /// Send text into the websocket
    ///
    /// ```rust,ignore
    /// if session.text("Some text").await.is_err() {
    ///     // session closed
    /// }
    /// ```
    pub async fn text(&mut self, msg: impl Into<ByteString>) -> Result<(), Closed> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            inner
                .send(Message::Text(msg.into()))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    /// Send raw bytes into the websocket
    ///
    /// ```rust,ignore
    /// if session.binary(b"some bytes").await.is_err() {
    ///     // session closed
    /// }
    /// ```
    pub async fn binary(&mut self, msg: impl Into<Bytes>) -> Result<(), Closed> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            inner
                .send(Message::Binary(msg.into()))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    /// Ping the client
    ///
    /// For many applications, it will be important to send regular pings to keep track of if the
    /// client has disconnected
    ///
    /// ```rust,ignore
    /// if session.ping(b"").await.is_err() {
    ///     // session is closed
    /// }
    /// ```
    pub async fn ping(&mut self, msg: &[u8]) -> Result<(), Closed> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            inner
                .send(Message::Ping(Bytes::copy_from_slice(msg)))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    /// Pong the client
    ///
    /// ```rust,ignore
    /// match msg {
    ///     Message::Ping(bytes) => {
    ///         let _ = session.pong(&bytes).await;
    ///     }
    ///     _ => (),
    /// }
    pub async fn pong(&mut self, msg: &[u8]) -> Result<(), Closed> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            inner
                .send(Message::Pong(Bytes::copy_from_slice(msg)))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    /// Send a close message, and consume the session
    ///
    /// All clones will return `Err(Closed)` if used after this call
    ///
    /// ```rust,ignore
    /// session.close(None).await
    /// ```
    pub async fn close(mut self, reason: Option<CloseReason>) -> Result<(), Closed> {
        self.pre_check();
        if let Some(inner) = self.inner.take() {
            self.closed.store(true, Ordering::Relaxed);
            inner.send(Message::Close(reason)).await.map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }
}
