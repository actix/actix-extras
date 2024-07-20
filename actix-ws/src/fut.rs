use std::{
    collections::VecDeque,
    future::poll_fn,
    io, mem,
    pin::Pin,
    task::{Context, Poll},
};

use actix_codec::{Decoder, Encoder};
use actix_http::{
    ws::{CloseReason, Codec, Frame, Item, Message, ProtocolError},
    Payload,
};
use actix_web::{
    web::{Bytes, BytesMut},
    Error,
};
use bytestring::ByteString;
use futures_core::stream::Stream;
use tokio::sync::mpsc::Receiver;

/// A response body for Websocket HTTP Requests
pub struct StreamingBody {
    session_rx: Receiver<Message>,

    messages: VecDeque<Message>,
    buf: BytesMut,
    codec: Codec,
    closing: bool,
}

/// A stream of Messages from a websocket client
///
/// Messages can be accessed via the stream's `.next()` method
pub struct MessageStream {
    payload: Payload,

    messages: VecDeque<Message>,
    buf: BytesMut,
    codec: Codec,
    closing: bool,
}

/// A Websocket message with continuations aggregated together
#[derive(Debug, PartialEq, Eq)]
pub enum AggregatedMessage {
    /// Text message
    Text(ByteString),

    /// Binary message
    Binary(Bytes),

    /// Ping message
    Ping(Bytes),

    /// Pong message
    Pong(Bytes),

    /// Close message with optional reason
    Close(Option<CloseReason>),
}

enum ContinuationKind {
    Text,
    Binary,
}

/// A stream of Messages from a websocket client
///
/// This stream aggregates Continuation frames into their equivalent combined forms, e.g. Binary or
/// Text.
pub struct AggregatedMessageStream {
    stream: MessageStream,

    current_size: usize,
    max_size: usize,
    continuations: Vec<Bytes>,
    continuation_kind: ContinuationKind,
}

impl StreamingBody {
    pub(super) fn new(session_rx: Receiver<Message>) -> Self {
        StreamingBody {
            session_rx,
            messages: VecDeque::new(),
            buf: BytesMut::new(),
            codec: Codec::new(),
            closing: false,
        }
    }
}

impl MessageStream {
    pub(super) fn new(payload: Payload) -> Self {
        MessageStream {
            payload,
            messages: VecDeque::new(),
            buf: BytesMut::new(),
            codec: Codec::new(),
            closing: false,
        }
    }

    /// Set the maximum permitted websocket frame size for received frames
    ///
    /// The `max_size` unit is `bytes`
    /// The default value for `max_size` is 65_536, or 64KB
    ///
    /// Any received frames larger than the permitted value will return
    /// `Err(ProtocolError::Overflow)` instead.
    ///
    /// ```rust,no_run
    /// # use actix_ws::MessageStream;
    /// # fn test(stream: MessageStream) {
    /// // Increase permitted frame size from 64KB to 1MB
    /// let stream = stream.max_frame_size(1024 * 1024);
    /// # }
    /// ```
    pub fn max_frame_size(self, max_size: usize) -> Self {
        Self {
            codec: self.codec.max_size(max_size),
            ..self
        }
    }

    /// Produce a stream that collects Continuation frames into their equivalent collected forms,
    /// e.g. Binary or Text.
    ///
    /// By default, continuations will be aggregated up to 1MB in size, erroring if the size is
    /// exceeded.
    pub fn aggregate_continuations(self) -> AggregatedMessageStream {
        AggregatedMessageStream {
            stream: self,

            current_size: 0,
            max_size: 1024 * 1024,
            continuations: Vec::new(),
            continuation_kind: ContinuationKind::Binary,
        }
    }

    /// Wait for the next item from the message stream
    ///
    /// ```no_run
    /// # use actix_ws::MessageStream;
    /// # async fn test(mut stream: MessageStream) {
    /// while let Some(Ok(msg)) = stream.recv().await {
    ///     // handle message
    /// }
    /// # }
    /// ```
    pub async fn recv(&mut self) -> Option<Result<Message, ProtocolError>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }
}

impl AggregatedMessageStream {
    /// Set the maximum allowed size for aggregated continuations.
    ///
    /// ```rust,no_run
    /// # use actix_ws::AggregatedMessageStream;
    /// # async fn test(stream: AggregatedMessageStream) {
    /// // Increase the allowed size from 1MB to 8MB
    /// let mut stream = stream.max_continuation_size(1024 * 1024 * 8);
    ///
    /// while let Some(Ok(msg)) = stream.recv().await {
    ///     // handle message
    /// }
    /// # }
    /// ```
    pub fn max_continuation_size(self, max_size: usize) -> Self {
        Self { max_size, ..self }
    }

    /// Wait for the next item from the message stream
    ///
    /// ```rust,no_run
    /// # use actix_ws::AggregatedMessageStream;
    /// # async fn test(mut stream: AggregatedMessageStream) {
    /// while let Some(Ok(msg)) = stream.recv().await {
    ///     // handle message
    /// }
    /// # }
    /// ```
    pub async fn recv(&mut self) -> Option<Result<AggregatedMessage, ProtocolError>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }
}

impl Stream for StreamingBody {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.closing {
            return Poll::Ready(None);
        }

        loop {
            match Pin::new(&mut this.session_rx).poll_recv(cx) {
                Poll::Ready(Some(msg)) => {
                    this.messages.push_back(msg);
                }
                Poll::Ready(None) => {
                    this.closing = true;
                    break;
                }
                Poll::Pending => break,
            }
        }

        while let Some(msg) = this.messages.pop_front() {
            if let Err(err) = this.codec.encode(msg, &mut this.buf) {
                return Poll::Ready(Some(Err(err.into())));
            }
        }

        if !this.buf.is_empty() {
            return Poll::Ready(Some(Ok(mem::take(&mut this.buf).freeze())));
        }

        Poll::Pending
    }
}

impl Stream for MessageStream {
    type Item = Result<Message, ProtocolError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // Return the first message in the queue if one exists
        //
        // This is faster than polling and parsing
        if let Some(msg) = this.messages.pop_front() {
            return Poll::Ready(Some(Ok(msg)));
        }

        if !this.closing {
            // Read in bytes until there's nothing left to read
            loop {
                match Pin::new(&mut this.payload).poll_next(cx) {
                    Poll::Ready(Some(Ok(bytes))) => {
                        this.buf.extend_from_slice(&bytes);
                    }
                    Poll::Ready(Some(Err(e))) => {
                        return Poll::Ready(Some(Err(ProtocolError::Io(io::Error::new(
                            io::ErrorKind::Other,
                            e.to_string(),
                        )))));
                    }
                    Poll::Ready(None) => {
                        this.closing = true;
                        break;
                    }
                    Poll::Pending => break,
                }
            }
        }

        // Create messages until there's no more bytes left
        while let Some(frame) = this.codec.decode(&mut this.buf)? {
            let message = match frame {
                Frame::Text(bytes) => {
                    let s = std::str::from_utf8(&bytes)
                        .map_err(|e| {
                            ProtocolError::Io(io::Error::new(io::ErrorKind::Other, e.to_string()))
                        })?
                        .to_string();
                    Message::Text(s.into())
                }
                Frame::Binary(bytes) => Message::Binary(bytes),
                Frame::Ping(bytes) => Message::Ping(bytes),
                Frame::Pong(bytes) => Message::Pong(bytes),
                Frame::Close(reason) => Message::Close(reason),
                Frame::Continuation(item) => Message::Continuation(item),
            };

            this.messages.push_back(message);
        }

        // Return the first message in the queue
        if let Some(msg) = this.messages.pop_front() {
            return Poll::Ready(Some(Ok(msg)));
        }

        // If we've exhausted our message queue and we're closing, close the stream
        if this.closing {
            return Poll::Ready(None);
        }

        Poll::Pending
    }
}

fn collect(continuations: &mut Vec<Bytes>) -> Bytes {
    let continuations = std::mem::take(continuations);
    let total_len = continuations.iter().map(|b| b.len()).sum();

    let mut collected = BytesMut::with_capacity(total_len);

    for b in continuations {
        collected.extend(b);
    }

    collected.freeze()
}

fn size_error() -> Poll<Option<Result<AggregatedMessage, ProtocolError>>> {
    Poll::Ready(Some(Err(ProtocolError::Io(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Exceeded maximum continuation size",
    )))))
}

impl Stream for AggregatedMessageStream {
    type Item = Result<AggregatedMessage, ProtocolError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        match std::task::ready!(Pin::new(&mut this.stream).poll_next(cx)) {
            Some(Ok(Message::Continuation(item))) => match item {
                Item::FirstText(bytes) => {
                    this.continuation_kind = ContinuationKind::Text;
                    this.current_size += bytes.len();

                    if this.current_size > this.max_size {
                        this.continuations.clear();

                        return size_error();
                    }

                    this.continuations.push(bytes);

                    Poll::Pending
                }
                Item::FirstBinary(bytes) => {
                    this.continuation_kind = ContinuationKind::Binary;
                    this.current_size += bytes.len();

                    if this.current_size > this.max_size {
                        this.continuations.clear();

                        return size_error();
                    }

                    this.continuations.push(bytes);

                    Poll::Pending
                }
                Item::Continue(bytes) => {
                    this.current_size += bytes.len();

                    if this.current_size > this.max_size {
                        this.continuations.clear();

                        return size_error();
                    }

                    this.continuations.push(bytes);

                    Poll::Pending
                }
                Item::Last(bytes) => {
                    this.current_size += bytes.len();

                    if this.current_size > this.max_size {
                        // reset current_size, as this is the last message for the current
                        // continuation
                        this.current_size = 0;
                        this.continuations.clear();

                        return size_error();
                    }

                    this.continuations.push(bytes);
                    let bytes = collect(&mut this.continuations);

                    this.current_size = 0;

                    match this.continuation_kind {
                        ContinuationKind::Text => {
                            match std::str::from_utf8(&bytes) {
                                Ok(_) => {
                                    // SAFETY: just checked valid UTF8 above
                                    let bytestring =
                                        unsafe { ByteString::from_bytes_unchecked(bytes) };
                                    Poll::Ready(Some(Ok(AggregatedMessage::Text(bytestring))))
                                }
                                Err(e) => Poll::Ready(Some(Err(ProtocolError::Io(
                                    io::Error::new(io::ErrorKind::Other, e.to_string()),
                                )))),
                            }
                        }
                        ContinuationKind::Binary => {
                            Poll::Ready(Some(Ok(AggregatedMessage::Binary(bytes))))
                        }
                    }
                }
            },
            Some(Ok(Message::Text(text))) => Poll::Ready(Some(Ok(AggregatedMessage::Text(text)))),
            Some(Ok(Message::Binary(binary))) => {
                Poll::Ready(Some(Ok(AggregatedMessage::Binary(binary))))
            }
            Some(Ok(Message::Ping(ping))) => Poll::Ready(Some(Ok(AggregatedMessage::Ping(ping)))),
            Some(Ok(Message::Pong(pong))) => Poll::Ready(Some(Ok(AggregatedMessage::Pong(pong)))),
            Some(Ok(Message::Close(close))) => {
                Poll::Ready(Some(Ok(AggregatedMessage::Close(close))))
            }
            Some(Ok(Message::Nop)) => unimplemented!("MessageStream cannot produce Nops"),
            Some(Err(e)) => Poll::Ready(Some(Err(e))),
            None => Poll::Ready(None),
        }
    }
}
