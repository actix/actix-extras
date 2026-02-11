//! Typed WebSocket messages via pluggable codecs.
//!
//! This module provides a small framework for doing that. Concrete codecs can be
//! implemented by user code or enabled via crate features.
//!
//! # Feature Flags
//!
//! - `serde-json`: enables the `JsonCodec` type (requires `serde` + `serde_json`).

use std::{
    fmt,
    future::poll_fn,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use actix_http::ws::{CloseReason, ProtocolError};
use actix_web::web::Bytes;
use bytestring::ByteString;
use futures_core::Stream;

use crate::{AggregatedMessage, AggregatedMessageStream, Closed, MessageStream, Session};

#[cfg(feature = "serde-json")]
mod json;

#[cfg(feature = "serde-json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde-json")))]
pub use self::json::JsonCodec;

/// A codec that can translate between typed values and WebSocket messages.
pub trait MessageCodec<T> {
    /// Codec-specific error type.
    type Error;

    /// Encodes a value into a WebSocket text or binary message.
    fn encode(&self, item: &T) -> Result<EncodedMessage, Self::Error>;

    /// Decodes an incoming WebSocket message into a typed value or a control message.
    fn decode(&self, msg: AggregatedMessage) -> Result<CodecMessage<T>, Self::Error>;
}

/// WebSocket messages that can be sent by a codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodedMessage {
    /// Text message.
    Text(ByteString),

    /// Binary message.
    Binary(Bytes),
}

/// Typed message yielded by a [`CodecMessageStream`].
#[derive(Debug)]
pub enum CodecMessage<T> {
    /// Successfully decoded application message.
    Item(T),

    /// Ping message.
    Ping(Bytes),

    /// Pong message.
    Pong(Bytes),

    /// Close message with optional reason.
    Close(Option<CloseReason>),
}

/// Errors returned by [`CodecSession::send()`].
#[derive(Debug)]
pub enum CodecSendError<E> {
    /// The session is closed.
    Closed(Closed),

    /// The codec failed to encode the outgoing value.
    Codec(E),
}

impl<E> fmt::Display for CodecSendError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecSendError::Closed(_) => f.write_str("session is closed"),
            CodecSendError::Codec(err) => write!(f, "codec error: {err}"),
        }
    }
}

impl<E> std::error::Error for CodecSendError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CodecSendError::Closed(err) => Some(err),
            CodecSendError::Codec(err) => Some(err),
        }
    }
}

/// Errors returned by [`CodecMessageStream`].
#[derive(Debug)]
pub enum CodecStreamError<E> {
    /// The WebSocket stream failed to decode frames.
    Protocol(ProtocolError),

    /// The codec failed to decode an application message.
    Codec(E),
}

impl<E> fmt::Display for CodecStreamError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecStreamError::Protocol(err) => write!(f, "protocol error: {err}"),
            CodecStreamError::Codec(err) => write!(f, "codec error: {err}"),
        }
    }
}

impl<E> std::error::Error for CodecStreamError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CodecStreamError::Protocol(err) => Some(err),
            CodecStreamError::Codec(err) => Some(err),
        }
    }
}

/// A [`Session`] wrapper that can send typed messages using a codec.
pub struct CodecSession<T, C> {
    session: Session,
    codec: C,
    _phantom: PhantomData<fn() -> T>,
}

impl<T, C> CodecSession<T, C>
where
    C: MessageCodec<T>,
{
    /// Constructs a new codec session wrapper.
    pub fn new(session: Session, codec: C) -> Self {
        Self {
            session,
            codec,
            _phantom: PhantomData,
        }
    }

    /// Returns a reference to the underlying session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Returns a mutable reference to the underlying session.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Returns a reference to the underlying codec.
    pub fn codec(&self) -> &C {
        &self.codec
    }

    /// Returns a mutable reference to the underlying codec.
    pub fn codec_mut(&mut self) -> &mut C {
        &mut self.codec
    }

    /// Consumes this wrapper and returns the underlying [`Session`].
    pub fn into_inner(self) -> Session {
        self.session
    }

    /// Encodes `item` and sends it as a WebSocket message.
    ///
    /// This method only sends text or binary frames. Use the underlying [`Session`] for control
    /// frames (ping/pong/close).
    pub async fn send(&mut self, item: &T) -> Result<(), CodecSendError<C::Error>> {
        let msg = self.codec.encode(item).map_err(CodecSendError::Codec)?;

        match msg {
            EncodedMessage::Text(text) => self
                .session
                .text(text)
                .await
                .map_err(CodecSendError::Closed),

            EncodedMessage::Binary(bin) => self
                .session
                .binary(bin)
                .await
                .map_err(CodecSendError::Closed),
        }
    }

    /// Sends a close frame, consuming the codec session.
    pub async fn close(self, reason: Option<CloseReason>) -> Result<(), Closed> {
        self.session.close(reason).await
    }
}

/// A [`Stream`] of typed messages decoded from an [`AggregatedMessageStream`].
pub struct CodecMessageStream<T, C> {
    stream: AggregatedMessageStream,
    codec: C,
    _phantom: PhantomData<fn() -> T>,
}

impl<T, C> CodecMessageStream<T, C>
where
    C: MessageCodec<T>,
{
    /// Constructs a new codec message stream wrapper.
    pub fn new(stream: AggregatedMessageStream, codec: C) -> Self {
        Self {
            stream,
            codec,
            _phantom: PhantomData,
        }
    }

    /// Returns a reference to the underlying codec.
    pub fn codec(&self) -> &C {
        &self.codec
    }

    /// Returns a mutable reference to the underlying codec.
    pub fn codec_mut(&mut self) -> &mut C {
        &mut self.codec
    }

    /// Consumes this wrapper and returns the underlying stream.
    pub fn into_inner(self) -> AggregatedMessageStream {
        self.stream
    }

    /// Waits for the next item from the codec message stream.
    ///
    /// This is a convenience for calling the [`Stream`](Stream::poll_next()) implementation.
    #[must_use]
    pub async fn recv(&mut self) -> Option<<Self as Stream>::Item> {
        // `CodecMessageStream` is not necessarily `Unpin` (depends on codec type) but it is safe
        // to pin it for the duration of this future since it is borrowed for the await.
        poll_fn(|cx| unsafe { Pin::new_unchecked(&mut *self) }.poll_next(cx)).await
    }
}

impl<T, C> Stream for CodecMessageStream<T, C>
where
    C: MessageCodec<T>,
{
    type Item = Result<CodecMessage<T>, CodecStreamError<C::Error>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // SAFETY: We will not move out of any fields. `AggregatedMessageStream` is polled by
        // pinning its field, and the codec is only accessed by reference.
        let this = unsafe { self.get_unchecked_mut() };

        let msg = match Pin::new(&mut this.stream).poll_next(cx) {
            Poll::Ready(Some(Ok(msg))) => msg,
            Poll::Ready(Some(Err(err))) => {
                return Poll::Ready(Some(Err(CodecStreamError::Protocol(err))));
            }
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => return Poll::Pending,
        };

        match this.codec.decode(msg) {
            Ok(item) => Poll::Ready(Some(Ok(item))),
            Err(err) => Poll::Ready(Some(Err(CodecStreamError::Codec(err)))),
        }
    }
}

impl MessageStream {
    /// Wraps this message stream with `codec`, aggregating continuation frames before decoding.
    #[must_use]
    pub fn with_codec<T, C>(self, codec: C) -> CodecMessageStream<T, C>
    where
        C: MessageCodec<T>,
    {
        self.aggregate_continuations().with_codec(codec)
    }
}

impl AggregatedMessageStream {
    /// Wraps this aggregated message stream with `codec`.
    #[must_use]
    pub fn with_codec<T, C>(self, codec: C) -> CodecMessageStream<T, C>
    where
        C: MessageCodec<T>,
    {
        CodecMessageStream::new(self, codec)
    }
}

impl Session {
    /// Wraps this session with `codec` so it can send typed messages.
    #[must_use]
    pub fn with_codec<T, C>(self, codec: C) -> CodecSession<T, C>
    where
        C: MessageCodec<T>,
    {
        CodecSession::new(self, codec)
    }
}

#[cfg(all(test, feature = "serde-json"))]
mod tests {
    use actix_http::ws::Message;
    use actix_web::web::Bytes;
    use serde::{Deserialize, Serialize};

    use super::{CodecMessage, EncodedMessage};
    use crate::{codec::CodecStreamError, stream::tests::payload_pair, Session};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestMsg {
        a: u32,
    }

    #[tokio::test]
    async fn json_session_encodes_text_frames_by_default() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let session = Session::new(tx);

        let mut session = session.with_codec::<TestMsg, _>(crate::codec::JsonCodec::default());
        session.send(&TestMsg { a: 123 }).await.unwrap();

        match rx.recv().await.unwrap() {
            Message::Text(text) => {
                let s: &str = text.as_ref();
                assert_eq!(s, r#"{"a":123}"#);
            }
            other => panic!("expected text frame, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn json_session_can_encode_binary_frames() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let session = Session::new(tx);

        let mut session =
            session.with_codec::<TestMsg, _>(crate::codec::JsonCodec::default().binary());
        session.send(&TestMsg { a: 123 }).await.unwrap();

        match rx.recv().await.unwrap() {
            Message::Binary(bytes) => assert_eq!(bytes, Bytes::from_static(br#"{"a":123}"#)),
            other => panic!("expected binary frame, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn json_stream_decodes_text_and_binary_frames() {
        let (mut tx, rx) = payload_pair(8);
        let mut stream = crate::MessageStream::new(rx)
            .with_codec::<TestMsg, _>(crate::codec::JsonCodec::default());

        tx.send(Message::Text(r#"{"a":1}"#.into())).await;
        match stream.recv().await.unwrap().unwrap() {
            CodecMessage::Item(TestMsg { a }) => assert_eq!(a, 1),
            other => panic!("expected decoded item, got: {other:?}"),
        }

        tx.send(Message::Binary(Bytes::from_static(br#"{"a":2}"#)))
            .await;
        match stream.recv().await.unwrap().unwrap() {
            CodecMessage::Item(TestMsg { a }) => assert_eq!(a, 2),
            other => panic!("expected decoded item, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn json_stream_passes_through_control_frames() {
        let (mut tx, rx) = payload_pair(8);
        let mut stream = crate::MessageStream::new(rx)
            .with_codec::<TestMsg, _>(crate::codec::JsonCodec::default());

        tx.send(Message::Ping(Bytes::from_static(b"hi"))).await;
        match stream.recv().await.unwrap().unwrap() {
            CodecMessage::Ping(bytes) => assert_eq!(bytes, Bytes::from_static(b"hi")),
            other => panic!("expected ping, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn json_stream_yields_codec_error_on_invalid_payload_and_continues() {
        let (mut tx, rx) = payload_pair(8);
        let mut stream = crate::MessageStream::new(rx)
            .with_codec::<TestMsg, _>(crate::codec::JsonCodec::default());

        tx.send(Message::Text("not json".into())).await;
        match stream.recv().await.unwrap() {
            Err(CodecStreamError::Codec(_)) => {}
            other => panic!("expected codec error, got: {other:?}"),
        }

        tx.send(Message::Text(r#"{"a":9}"#.into())).await;
        match stream.recv().await.unwrap().unwrap() {
            CodecMessage::Item(TestMsg { a }) => assert_eq!(a, 9),
            other => panic!("expected decoded item, got: {other:?}"),
        }
    }

    #[test]
    fn encoded_message_is_lightweight() {
        let _ = EncodedMessage::Text("hello".into());
        let _ = EncodedMessage::Binary(Bytes::from_static(b"hello"));
    }
}
