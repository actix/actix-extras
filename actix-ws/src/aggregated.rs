//! WebSocket stream for aggregating continuation frames.

use std::{
    future::poll_fn,
    io, mem,
    pin::Pin,
    task::{ready, Context, Poll},
};

use actix_http::ws::{CloseReason, Item, Message, ProtocolError};
use actix_web::web::{Bytes, BytesMut};
use bytestring::ByteString;
use futures_core::Stream;

use crate::MessageStream;

pub(crate) enum ContinuationKind {
    Text,
    Binary,
}

/// WebSocket message with any continuations aggregated together.
#[derive(Debug, PartialEq, Eq)]
pub enum AggregatedMessage {
    /// Text message.
    Text(ByteString),

    /// Binary message.
    Binary(Bytes),

    /// Ping message.
    Ping(Bytes),

    /// Pong message.
    Pong(Bytes),

    /// Close message with optional reason.
    Close(Option<CloseReason>),
}

/// Stream of messages from a WebSocket client, with continuations aggregated.
pub struct AggregatedMessageStream {
    stream: MessageStream,
    current_size: usize,
    max_size: usize,
    continuations: Vec<Bytes>,
    continuation_kind: ContinuationKind,
}

impl AggregatedMessageStream {
    #[must_use]
    pub(crate) fn new(stream: MessageStream) -> Self {
        AggregatedMessageStream {
            stream,
            current_size: 0,
            max_size: 1024 * 1024,
            continuations: Vec::new(),
            continuation_kind: ContinuationKind::Binary,
        }
    }

    /// Sets the maximum allowed size for aggregated continuations, in bytes.
    ///
    /// By default, up to 1 MiB is allowed.
    ///
    /// ```no_run
    /// # use actix_ws::AggregatedMessageStream;
    /// # async fn test(stream: AggregatedMessageStream) {
    /// // increase the allowed size from 1MB to 8MB
    /// let mut stream = stream.max_continuation_size(8 * 1024 * 1024);
    ///
    /// while let Some(Ok(msg)) = stream.recv().await {
    ///     // handle message
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn max_continuation_size(mut self, max_size: usize) -> Self {
        self.max_size = max_size;
        self
    }

    /// Waits for the next item from the aggregated message stream.
    ///
    /// This is a convenience for calling the [`Stream`](Stream::poll_next()) implementation.
    ///
    /// ```no_run
    /// # use actix_ws::AggregatedMessageStream;
    /// # async fn test(mut stream: AggregatedMessageStream) {
    /// while let Some(Ok(msg)) = stream.recv().await {
    ///     // handle message
    /// }
    /// # }
    /// ```
    #[must_use]
    pub async fn recv(&mut self) -> Option<<Self as Stream>::Item> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }
}

fn size_error() -> Poll<Option<Result<AggregatedMessage, ProtocolError>>> {
    Poll::Ready(Some(Err(ProtocolError::Io(io::Error::other(
        "Exceeded maximum continuation size",
    )))))
}

impl Stream for AggregatedMessageStream {
    type Item = Result<AggregatedMessage, ProtocolError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        let Some(msg) = ready!(Pin::new(&mut this.stream).poll_next(cx)?) else {
            return Poll::Ready(None);
        };

        match msg {
            Message::Continuation(item) => match item {
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
                        // reset current_size, as this is the last message for
                        // the current continuation
                        this.current_size = 0;
                        this.continuations.clear();

                        return size_error();
                    }

                    this.continuations.push(bytes);
                    let bytes = collect(&mut this.continuations);

                    this.current_size = 0;

                    match this.continuation_kind {
                        ContinuationKind::Text => {
                            Poll::Ready(Some(match ByteString::try_from(bytes) {
                                Ok(bytestring) => Ok(AggregatedMessage::Text(bytestring)),
                                Err(err) => Err(ProtocolError::Io(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    err.to_string(),
                                ))),
                            }))
                        }
                        ContinuationKind::Binary => {
                            Poll::Ready(Some(Ok(AggregatedMessage::Binary(bytes))))
                        }
                    }
                }
            },

            Message::Text(text) => Poll::Ready(Some(Ok(AggregatedMessage::Text(text)))),
            Message::Binary(binary) => Poll::Ready(Some(Ok(AggregatedMessage::Binary(binary)))),
            Message::Ping(ping) => Poll::Ready(Some(Ok(AggregatedMessage::Ping(ping)))),
            Message::Pong(pong) => Poll::Ready(Some(Ok(AggregatedMessage::Pong(pong)))),
            Message::Close(close) => Poll::Ready(Some(Ok(AggregatedMessage::Close(close)))),

            Message::Nop => unreachable!("MessageStream should not produce no-ops"),
        }
    }
}

fn collect(continuations: &mut Vec<Bytes>) -> Bytes {
    let continuations = mem::take(continuations);
    let total_len = continuations.iter().map(|b| b.len()).sum();

    let mut buf = BytesMut::with_capacity(total_len);

    for chunk in continuations {
        buf.extend(chunk);
    }

    buf.freeze()
}
