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

        // If close has been initiated and there is no pending buffered data, end the stream.
        if this.closing && this.buf.is_empty() {
            return Poll::Ready(None);
        }

        if !this.closing {
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
        }

        while let Some(msg) = this.messages.pop_front() {
            let is_close = matches!(msg, Message::Close(_));

            if let Err(err) = this.codec.encode(msg, &mut this.buf) {
                return Poll::Ready(Some(Err(err.into())));
            }

            if is_close {
                // A WebSocket Close frame is terminal. End the response body after flushing this
                // frame, even if there are still `Session` clones holding the sender.
                this.closing = true;
                this.session_rx.close();
                this.messages.clear();
                break;
            }
        }

        if !this.buf.is_empty() {
            // Avoid retaining an ever-growing buffer after large payloads:
            // https://github.com/actix/actix-extras/commit/81954844158c27de3aa034d1b727d1c13753f325
            return Poll::Ready(Some(Ok(mem::take(&mut this.buf).freeze())));
        }

        if this.closing {
            return Poll::Ready(None);
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
                        if this.buf.is_empty() {
                            // Avoid a copy when there is no buffered data.
                            this.buf = BytesMut::from(bytes);
                        } else {
                            this.buf.extend_from_slice(&bytes);
                        }
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

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        future::Future,
        pin::Pin,
        task::{ready, Context, Poll},
    };

    use actix_http::error::PayloadError;
    use futures_core::Stream;
    use tokio::sync::mpsc::{Receiver, Sender};

    use super::{Bytes, BytesMut, Codec, Encoder, Message, MessageStream, Payload, StreamingBody};

    pub(crate) struct PayloadReceiver {
        rx: Receiver<Bytes>,
    }
    pub(crate) struct PayloadSender {
        codec: Codec,
        tx: Sender<Bytes>,
    }
    impl PayloadSender {
        pub(crate) async fn send(&mut self, message: Message) {
            self.send_many(vec![message]).await
        }
        pub(crate) async fn send_many(&mut self, messages: Vec<Message>) {
            let mut buf = BytesMut::new();

            for message in messages {
                self.codec.encode(message, &mut buf).unwrap();
            }

            self.tx.send(buf.freeze()).await.unwrap()
        }
    }
    impl Stream for PayloadReceiver {
        type Item = Result<Bytes, PayloadError>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let opt = ready!(self.get_mut().rx.poll_recv(cx));

            Poll::Ready(opt.map(Ok))
        }
    }
    pub(crate) fn payload_pair(capacity: usize) -> (PayloadSender, Payload) {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);

        (
            PayloadSender {
                codec: Codec::new().client_mode(),
                tx,
            },
            Payload::Stream {
                payload: Box::pin(PayloadReceiver { rx }),
            },
        )
    }

    #[tokio::test]
    async fn message_stream_yields_messages() {
        std::future::poll_fn(move |cx| {
            let (mut tx, rx) = payload_pair(8);
            let message_stream = MessageStream::new(rx);
            let mut stream = std::pin::pin!(message_stream);

            let messages = [
                Message::Binary(Bytes::from(vec![0, 1, 2, 3])),
                Message::Ping(Bytes::from(vec![3, 2, 1, 0])),
                Message::Close(None),
            ];

            for msg in messages {
                let poll = stream.as_mut().poll_next(cx);
                assert!(
                    poll.is_pending(),
                    "Stream should be pending when no messages are present {poll:?}"
                );

                let fut = tx.send(msg);
                let fut = std::pin::pin!(fut);

                assert!(fut.poll(cx).is_ready(), "Sending should not yield");
                assert!(
                    stream.as_mut().poll_next(cx).is_ready(),
                    "Stream should be ready"
                );
            }

            assert!(
                stream.as_mut().poll_next(cx).is_pending(),
                "Stream should be pending after processing messages"
            );

            Poll::Ready(())
        })
        .await
    }

    #[tokio::test]
    async fn message_stream_yields_consecutive_messages() {
        std::future::poll_fn(move |cx| {
            let (mut tx, rx) = payload_pair(8);
            let message_stream = MessageStream::new(rx);
            let mut stream = std::pin::pin!(message_stream);

            let messages = vec![
                Message::Binary(Bytes::from(vec![0, 1, 2, 3])),
                Message::Ping(Bytes::from(vec![3, 2, 1, 0])),
                Message::Close(None),
            ];

            let size = messages.len();

            let fut = tx.send_many(messages);
            let fut = std::pin::pin!(fut);
            assert!(fut.poll(cx).is_ready(), "Sending should not yield");

            for _ in 0..size {
                assert!(
                    stream.as_mut().poll_next(cx).is_ready(),
                    "Stream should be ready"
                );
            }

            assert!(
                stream.as_mut().poll_next(cx).is_pending(),
                "Stream should be pending after processing messages"
            );

            Poll::Ready(())
        })
        .await
    }

    #[tokio::test]
    async fn message_stream_closes() {
        std::future::poll_fn(move |cx| {
            let (tx, rx) = payload_pair(8);
            drop(tx);
            let message_stream = MessageStream::new(rx);
            let mut stream = std::pin::pin!(message_stream);

            let poll = stream.as_mut().poll_next(cx);
            assert!(
                matches!(poll, Poll::Ready(None)),
                "Stream should be ready when closing {poll:?}"
            );

            Poll::Ready(())
        })
        .await
    }

    #[tokio::test]
    async fn stream_produces_bytes_from_messages() {
        std::future::poll_fn(move |cx| {
            let (tx, rx) = tokio::sync::mpsc::channel(1);

            let stream = StreamingBody::new(rx);

            let messages = [
                Message::Binary(Bytes::from(vec![0, 1, 2, 3])),
                Message::Ping(Bytes::from(vec![3, 2, 1, 0])),
                Message::Close(None),
            ];

            let mut stream = std::pin::pin!(stream);

            for msg in messages {
                assert!(
                    stream.as_mut().poll_next(cx).is_pending(),
                    "Stream should be pending when no messages are present"
                );

                let fut = tx.send(msg);
                let fut = std::pin::pin!(fut);

                assert!(fut.poll(cx).is_ready(), "Sending should not yield");
                assert!(
                    stream.as_mut().poll_next(cx).is_ready(),
                    "Stream should be ready"
                );
            }

            assert!(
                matches!(stream.as_mut().poll_next(cx), Poll::Ready(None)),
                "stream should close after processing close message"
            );

            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn stream_processes_many_consecutive_messages() {
        std::future::poll_fn(move |cx| {
            let (tx, rx) = tokio::sync::mpsc::channel(3);

            let stream = StreamingBody::new(rx);

            let messages = [
                Message::Binary(Bytes::from(vec![0, 1, 2, 3])),
                Message::Ping(Bytes::from(vec![3, 2, 1, 0])),
                Message::Close(None),
            ];

            let mut stream = std::pin::pin!(stream);

            assert!(stream.as_mut().poll_next(cx).is_pending());

            for msg in messages {
                let fut = tx.send(msg);
                let fut = std::pin::pin!(fut);
                assert!(fut.poll(cx).is_ready(), "Sending should not yield");
            }

            assert!(
                stream.as_mut().poll_next(cx).is_ready(),
                "Stream should be ready"
            );
            assert!(
                matches!(stream.as_mut().poll_next(cx), Poll::Ready(None)),
                "stream should close after processing close message"
            );

            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn stream_closes_after_close_message_even_if_sender_alive() {
        std::future::poll_fn(move |cx| {
            let (tx, rx) = tokio::sync::mpsc::channel(1);

            let stream = StreamingBody::new(rx);
            let mut stream = std::pin::pin!(stream);

            assert!(
                stream.as_mut().poll_next(cx).is_pending(),
                "stream should start pending"
            );

            // Send a Close frame but keep the sender alive (e.g. a `Session` clone held elsewhere).
            {
                let fut = tx.send(Message::Close(None));
                let fut = std::pin::pin!(fut);
                assert!(fut.poll(cx).is_ready(), "Sending should not yield");
            }

            assert!(
                stream.as_mut().poll_next(cx).is_ready(),
                "stream should yield close frame bytes"
            );

            let poll = stream.as_mut().poll_next(cx);
            assert!(
                matches!(poll, Poll::Ready(None)),
                "stream should close after close frame even if sender is still alive"
            );

            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn stream_closes() {
        std::future::poll_fn(move |cx| {
            let (tx, rx) = tokio::sync::mpsc::channel(3);

            drop(tx);
            let stream = StreamingBody::new(rx);

            let mut stream = std::pin::pin!(stream);

            let poll = stream.as_mut().poll_next(cx);

            assert!(
                matches!(poll, Poll::Ready(None)),
                "stream should close after dropped tx"
            );

            Poll::Ready(())
        })
        .await;
    }
}
