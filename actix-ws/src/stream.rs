use std::{
    collections::VecDeque,
    future::poll_fn,
    io, mem,
    pin::Pin,
    task::{Context, Poll},
};

use actix_codec::{Decoder, Encoder};
use actix_http::{
    ws::{Codec, Frame, Message, ProtocolError},
    Payload,
};
use actix_web::{
    web::{Bytes, BytesMut},
    Error,
};
use bytestring::ByteString;
use futures_core::stream::Stream;
use tokio::sync::mpsc::Receiver;

use crate::AggregatedMessageStream;

/// Response body for a WebSocket.
pub struct StreamingBody {
    session_rx: Receiver<Message>,
    messages: VecDeque<Message>,
    buf: BytesMut,
    codec: Codec,
    closing: bool,
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

/// Stream of messages from a WebSocket client.
pub struct MessageStream {
    payload: Payload,

    messages: VecDeque<Message>,
    buf: BytesMut,
    codec: Codec,
    closing: bool,
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

    /// Sets the maximum permitted size for received WebSocket frames, in bytes.
    ///
    /// By default, up to 64KiB is allowed.
    ///
    /// Any received frames larger than the permitted value will return
    /// `Err(ProtocolError::Overflow)` instead.
    ///
    /// ```no_run
    /// # use actix_ws::MessageStream;
    /// # fn test(stream: MessageStream) {
    /// // increase permitted frame size from 64KB to 1MB
    /// let stream = stream.max_frame_size(1024 * 1024);
    /// # }
    /// ```
    #[must_use]
    pub fn max_frame_size(mut self, max_size: usize) -> Self {
        self.codec = self.codec.max_size(max_size);
        self
    }

    /// Returns a stream wrapper that collects continuation frames into their equivalent aggregated
    /// forms, i.e., binary or text.
    ///
    /// By default, continuations will be aggregated up to 1MiB in size (customizable with
    /// [`AggregatedMessageStream::max_continuation_size()`]). The stream implementation returns an
    /// error if this size is exceeded.
    #[must_use]
    pub fn aggregate_continuations(self) -> AggregatedMessageStream {
        AggregatedMessageStream::new(self)
    }

    /// Waits for the next item from the message stream
    ///
    /// This is a convenience for calling the [`Stream`](Stream::poll_next()) implementation.
    ///
    /// ```no_run
    /// # use actix_ws::MessageStream;
    /// # async fn test(mut stream: MessageStream) {
    /// while let Some(Ok(msg)) = stream.recv().await {
    ///     // handle message
    /// }
    /// # }
    /// ```
    #[must_use]
    pub async fn recv(&mut self) -> Option<Result<Message, ProtocolError>> {
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
                    Poll::Ready(Some(Err(err))) => {
                        return Poll::Ready(Some(Err(ProtocolError::Io(io::Error::other(err)))));
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
                    ByteString::try_from(bytes)
                        .map(Message::Text)
                        .map_err(|err| {
                            ProtocolError::Io(io::Error::new(io::ErrorKind::InvalidData, err))
                        })?
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
