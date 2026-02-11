use std::{
    fmt,
    future::poll_fn,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use actix_http::ws::{CloseReason, Item, Message};
use actix_web::web::Bytes;
use bytestring::ByteString;
use futures_sink::Sink;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::PollSender;

// RFC 6455: Control frames MUST have payload length <= 125 bytes.
// Close payload is: 2-byte close code + optional UTF-8 reason, therefore the reason is <= 123 bytes.
// ref. https://www.rfc-editor.org/rfc/rfc6455.html#section-5.5
const MAX_CONTROL_PAYLOAD_BYTES: usize = 125;
const MAX_CLOSE_REASON_BYTES: usize = MAX_CONTROL_PAYLOAD_BYTES - 2;

/// A handle into the websocket session.
///
/// This type can be used to send messages into the WebSocket.
/// It also implements [`Sink<Message>`](futures_sink::Sink) for integration with sink-based APIs.
#[derive(Clone)]
pub struct Session {
    inner: Option<PollSender<Message>>,
    closed: Arc<AtomicBool>,
}

/// The error representing a closed websocket session
#[derive(Debug)]
pub struct Closed;

impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Session is closed")
    }
}

impl std::error::Error for Closed {}

impl Session {
    pub(super) fn new(inner: Sender<Message>) -> Self {
        Session {
            inner: Some(PollSender::new(inner)),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn pre_check(&mut self) {
        if self.closed.load(Ordering::Relaxed) {
            self.inner.take();
        }
    }

    async fn send_message_inner(&mut self, msg: Message) -> Result<(), Closed> {
        if let Some(inner) = self.inner.as_mut() {
            poll_fn(|cx| Pin::new(&mut *inner).poll_ready(cx))
                .await
                .map_err(|_| Closed)?;
            Pin::new(&mut *inner).start_send(msg).map_err(|_| Closed)?;
            poll_fn(|cx| Pin::new(&mut *inner).poll_flush(cx))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    async fn send_message(&mut self, msg: Message) -> Result<(), Closed> {
        self.pre_check();
        self.send_message_inner(msg).await
    }

    /// Sends text into the WebSocket.
    ///
    /// ```no_run
    /// # use actix_ws::Session;
    /// # async fn test(mut session: Session) {
    /// if session.text("Some text").await.is_err() {
    ///     // session closed
    /// }
    /// # }
    /// ```
    pub async fn text(&mut self, msg: impl Into<ByteString>) -> Result<(), Closed> {
        self.send_message(Message::Text(msg.into())).await
    }

    /// Sends raw bytes into the WebSocket.
    ///
    /// ```no_run
    /// # use actix_ws::Session;
    /// # async fn test(mut session: Session) {
    /// if session.binary(&b"some bytes"[..]).await.is_err() {
    ///     // session closed
    /// }
    /// # }
    /// ```
    pub async fn binary(&mut self, msg: impl Into<Bytes>) -> Result<(), Closed> {
        self.send_message(Message::Binary(msg.into())).await
    }

    /// Pings the client.
    ///
    /// For many applications, it will be important to send regular pings to keep track of if the
    /// client has disconnected
    ///
    /// Ping payloads longer than 125 bytes are truncated to comply with RFC 6455 control frame
    /// size limits.
    ///
    /// ```no_run
    /// # use actix_ws::Session;
    /// # async fn test(mut session: Session) {
    /// if session.ping(b"").await.is_err() {
    ///     // session is closed
    /// }
    /// # }
    /// ```
    pub async fn ping(&mut self, msg: &[u8]) -> Result<(), Closed> {
        let msg = if msg.len() > MAX_CONTROL_PAYLOAD_BYTES {
            &msg[..MAX_CONTROL_PAYLOAD_BYTES]
        } else {
            msg
        };
        self.send_message(Message::Ping(Bytes::copy_from_slice(msg)))
            .await
    }

    /// Pongs the client.
    ///
    /// Pong payloads longer than 125 bytes are truncated to comply with RFC 6455 control frame
    /// size limits.
    ///
    /// ```no_run
    /// # use actix_ws::{Message, Session};
    /// # async fn test(mut session: Session, msg: Message) {
    /// match msg {
    ///     Message::Ping(bytes) => {
    ///         let _ = session.pong(&bytes).await;
    ///     }
    ///     _ => (),
    /// }
    /// # }
    pub async fn pong(&mut self, msg: &[u8]) -> Result<(), Closed> {
        let msg = if msg.len() > MAX_CONTROL_PAYLOAD_BYTES {
            &msg[..MAX_CONTROL_PAYLOAD_BYTES]
        } else {
            msg
        };
        self.send_message(Message::Pong(Bytes::copy_from_slice(msg)))
            .await
    }

    /// Manually controls sending continuations.
    ///
    /// Be wary of this method. Continuations represent multiple frames that, when combined, are
    /// presented as a single message. They are useful when the entire contents of a message are
    /// not available all at once. However, continuations MUST NOT be interrupted by other Text or
    /// Binary messages. Control messages such as Ping, Pong, or Close are allowed to interrupt a
    /// continuation.
    ///
    /// Continuations must be initialized with a First variant, and must be terminated by a Last
    /// variant, with only Continue variants sent in between.
    ///
    /// ```no_run
    /// # use actix_ws::{Item, Session};
    /// # async fn test(mut session: Session) -> Result<(), Box<dyn std::error::Error>> {
    /// session.continuation(Item::FirstText("Hello".into())).await?;
    /// session.continuation(Item::Continue(b", World"[..].into())).await?;
    /// session.continuation(Item::Last(b"!"[..].into())).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn continuation(&mut self, msg: Item) -> Result<(), Closed> {
        self.send_message(Message::Continuation(msg)).await
    }

    /// Sends a close message, and consumes the session.
    ///
    /// All clones will return `Err(Closed)` if used after this call.
    ///
    /// Close reason descriptions longer than 123 bytes are truncated to comply with RFC 6455
    /// control frame size limits.
    ///
    /// ```no_run
    /// # use actix_ws::{Closed, Session};
    /// # async fn test(mut session: Session) -> Result<(), Closed> {
    /// session.close(None).await
    /// # }
    /// ```
    pub async fn close(mut self, reason: Option<CloseReason>) -> Result<(), Closed> {
        self.pre_check();

        let mut reason = reason;

        if let Some(reason) = reason.as_mut() {
            if let Some(desc) = reason.description.as_mut() {
                if desc.len() > MAX_CLOSE_REASON_BYTES {
                    let mut end = MAX_CLOSE_REASON_BYTES;
                    while end > 0 && !desc.is_char_boundary(end) {
                        end -= 1;
                    }
                    desc.truncate(end);
                }
            }
        }

        if self.inner.is_some() {
            self.closed.store(true, Ordering::Relaxed);
            self.send_message_inner(Message::Close(reason)).await
        } else {
            Err(Closed)
        }
    }
}

impl Sink<Message> for Session {
    type Error = Closed;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            match Pin::new(inner).poll_ready(cx) {
                Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                Poll::Ready(Err(_)) => Poll::Ready(Err(Closed)),
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(Err(Closed))
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            Pin::new(inner).start_send(item).map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            match Pin::new(inner).poll_flush(cx) {
                Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                Poll::Ready(Err(_)) => Poll::Ready(Err(Closed)),
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(Err(Closed))
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.closed.store(true, Ordering::Relaxed);
        if let Some(inner) = self.inner.as_mut() {
            match Pin::new(inner).poll_close(cx) {
                Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                Poll::Ready(Err(_)) => Poll::Ready(Err(Closed)),
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_http::ws::Message;
    use futures_util::SinkExt;

    use super::Session;

    #[tokio::test]
    async fn session_implements_sink() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let mut session = Session::new(tx);

        session
            .send(Message::Text("hello from sink".into()))
            .await
            .unwrap();

        match rx.recv().await {
            Some(Message::Text(msg)) => {
                let text: &str = msg.as_ref();
                assert_eq!(text, "hello from sink");
            }
            other => panic!("expected text frame, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn sink_close_closes_all_clones() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let mut session = Session::new(tx);
        let mut clone = session.clone();

        SinkExt::close(&mut session).await.unwrap();
        assert!(clone.text("should fail").await.is_err());

        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn close_sends_close_frame_and_closes_all_clones() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let session = Session::new(tx);
        let mut clone = session.clone();

        session.close(None).await.unwrap();
        assert!(clone.text("should fail").await.is_err());

        match rx.recv().await {
            Some(Message::Close(None)) => {}
            other => panic!("expected close frame, got: {other:?}"),
        }
    }
}
