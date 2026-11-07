use std::{
    cell::RefCell,
    fmt,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use actix_http::ws::{CloseReason, Item, Message};
use actix_web::web::Bytes;
use bytestring::ByteString;
use futures_sink::Sink;
use local_channel::mpsc::Sender;

// RFC 6455: Control frames MUST have payload length <= 125 bytes.
// Close payload is: 2-byte close code + optional UTF-8 reason, therefore the reason is <= 123 bytes.
// ref. https://www.rfc-editor.org/rfc/rfc6455.html#section-5.5
const MAX_CONTROL_PAYLOAD_BYTES: usize = 125;
const MAX_CLOSE_REASON_BYTES: usize = MAX_CONTROL_PAYLOAD_BYTES - 2;

/// A handle into the websocket session.
///
/// This type can be used to send messages into the WebSocket.
/// It also implements [`Sink<Message>`](futures_sink::Sink) for integration with sink-based APIs.
///
/// Sessions and their clones must stay on the thread that handles the connection. They are
/// neither `Send` nor `Sync`. Use [`actix_web::rt::spawn`] to run a task on that thread.
///
/// Sending queues a message in an unbounded channel. It does not wait for the client to read
/// the message. Limit production for slow clients to prevent unbounded memory use. Sink readiness
/// and flush operations do not wait for network writes.
#[derive(Clone)]
pub struct Session {
    inner: Rc<RefCell<Option<Sender<Message>>>>,
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
            inner: Rc::new(RefCell::new(Some(inner))),
        }
    }

    async fn send_message(&mut self, msg: Message) -> Result<(), Closed> {
        self.inner
            .borrow()
            .as_ref()
            .ok_or(Closed)?
            .send(msg)
            .map_err(|_| Closed)
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

        self.send_message(Message::Close(reason)).await?;
        self.inner.borrow_mut().take();
        Ok(())
    }
}

impl Sink<Message> for Session {
    type Error = Closed;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Closed>> {
        Poll::Ready(if self.inner.borrow().is_some() {
            Ok(())
        } else {
            Err(Closed)
        })
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Closed> {
        self.inner
            .borrow()
            .as_ref()
            .ok_or(Closed)?
            .send(item)
            .map_err(|_| Closed)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Closed>> {
        self.poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Closed>> {
        // Drop the only channel sender so a pending receiver wakes and drains the queue.
        self.inner.borrow_mut().take();
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use actix_http::ws::Message;
    use futures_util::SinkExt;

    use super::Session;

    #[tokio::test]
    async fn session_implements_sink() {
        let (tx, mut rx) = local_channel::mpsc::channel();
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
    async fn sends_more_than_the_old_capacity_without_a_receiver_poll() {
        let (tx, mut rx) = local_channel::mpsc::channel();
        let mut session = Session::new(tx);

        for value in 0..64 {
            let send = session.text(value.to_string());
            let mut send = std::pin::pin!(send);

            assert!(matches!(
                std::future::Future::poll(
                    send.as_mut(),
                    &mut std::task::Context::from_waker(std::task::Waker::noop())
                ),
                std::task::Poll::Ready(Ok(()))
            ));
        }

        drop(session);

        for value in 0..64 {
            match rx.recv().await {
                Some(Message::Text(text)) => assert_eq!(text, value.to_string()),
                other => panic!("expected text frame, got: {other:?}"),
            }
        }

        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn sink_close_closes_all_clones() {
        let (tx, mut rx) = local_channel::mpsc::channel();
        let mut session = Session::new(tx);
        let mut clone = session.clone();

        SinkExt::close(&mut session).await.unwrap();
        assert!(clone.text("should fail").await.is_err());

        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn sink_close_drains_queued_messages_with_a_clone_alive() {
        let (tx, mut rx) = local_channel::mpsc::channel();
        let mut session = Session::new(tx);
        let mut clone = session.clone();

        session.text("first").await.unwrap();
        clone.text("second").await.unwrap();
        SinkExt::close(&mut session).await.unwrap();

        for expected in ["first", "second"] {
            match rx.recv().await {
                Some(Message::Text(text)) => assert_eq!(text, expected),
                other => panic!("expected text frame, got: {other:?}"),
            }
        }

        assert!(rx.recv().await.is_none());
        assert!(clone.text("closed").await.is_err());
    }

    #[tokio::test]
    async fn sends_fail_after_receiver_drop() {
        let (tx, rx) = local_channel::mpsc::channel();
        let mut session = Session::new(tx);
        let mut clone = session.clone();

        drop(rx);

        assert!(session.text("closed").await.is_err());
        assert!(clone.send(Message::Ping(Default::default())).await.is_err());
    }

    #[tokio::test]
    async fn sink_close_wakes_receiver_with_a_clone_alive() {
        use std::{
            sync::Arc,
            task::{Context, Poll, Wake, Waker},
        };

        struct WakeFlag(std::sync::atomic::AtomicBool);

        impl Wake for WakeFlag {
            fn wake(self: Arc<Self>) {
                self.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

        let (tx, mut rx) = local_channel::mpsc::channel();
        let mut session = Session::new(tx);
        let mut clone = session.clone();
        let flag = Arc::new(WakeFlag(std::sync::atomic::AtomicBool::new(false)));
        let waker = Waker::from(flag.clone());
        let mut cx = Context::from_waker(&waker);
        let mut receive = std::pin::pin!(rx.recv());

        assert!(std::future::Future::poll(receive.as_mut(), &mut cx).is_pending());

        SinkExt::close(&mut session).await.unwrap();

        assert!(flag.0.load(std::sync::atomic::Ordering::Relaxed));
        assert!(matches!(
            std::future::Future::poll(receive.as_mut(), &mut cx),
            Poll::Ready(None)
        ));
        assert!(clone.text("closed").await.is_err());
    }

    #[tokio::test]
    async fn close_sends_close_frame_and_closes_all_clones() {
        let (tx, mut rx) = local_channel::mpsc::channel();
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
