use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use actix_web::{
    HttpRequest, HttpResponse, Responder,
    body::{BodySize, BoxBody, InfallibleStream, MessageBody},
    http::header::{CacheControl, CacheDirective, ContentEncoding},
};
use bytes::Bytes;
use bytestring::ByteString;
use futures_util::Stream;
use pin_project_lite::pin_project;
use tokio::{
    sync::mpsc,
    time::{Interval, interval},
};
use tokio_stream::wrappers::ReceiverStream;

use crate::{BoxError, Event};

pin_project! {
    /// Server-sent events (`text/event-stream`) responder.
    ///
    /// Constructed using your [own stream](Self::from_stream).
    /// Wrap a Tokio receiver in [`ReceiverStream`] to use it as a stream.
    #[must_use]
    #[derive(Debug)]
    pub struct Sse<S> {
        #[pin]
        stream: S,
        keep_alive: Option<Interval>,
        retry_interval: Option<Duration>,
    }
}

impl<S, E> Sse<S>
where
    S: Stream<Item = Result<Event, E>> + 'static,
    E: Into<BoxError>,
{
    /// Create an SSE response from a stream that yields SSE [Event]s.
    pub fn from_stream(stream: S) -> Self {
        Self {
            stream,
            keep_alive: None,
            retry_interval: None,
        }
    }
}

impl<S> Sse<InfallibleStream<S>>
where
    S: Stream<Item = Event> + 'static,
{
    /// Create an SSE response from an infallible stream that yields SSE [Event]s.
    pub fn from_infallible_stream(stream: S) -> Self {
        Sse::from_stream(InfallibleStream::new(stream))
    }
}

impl Sse<InfallibleStream<ReceiverStream<Event>>> {
    /// Create an SSE response from a receiver that yields SSE [Event]s.
    pub fn from_infallible_receiver(receiver: mpsc::Receiver<Event>) -> Self {
        Self::from_stream(InfallibleStream::new(ReceiverStream::new(receiver)))
    }
}

impl<S> Sse<S> {
    /// Enables "keep-alive" messages to be sent in the event stream after a period of inactivity.
    ///
    /// By default, no keep-alive is set up.
    pub fn with_keep_alive(mut self, keep_alive_period: Duration) -> Self {
        let mut int = interval(keep_alive_period);
        int.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        self.keep_alive = Some(int);
        self
    }

    /// Queues an initial standalone `retry` frame to set the client's reconnection delay.
    ///
    /// The delay is sent in milliseconds and remains effective until another `retry` field replaces
    /// it. It does not retry individual message delivery. Use [`crate::Data::retry`] to update the delay in
    /// a data message.
    ///
    /// Browsers default to retry every 3 seconds or so.
    pub fn with_retry_duration(mut self, retry: Duration) -> Self {
        self.retry_interval = Some(retry);
        self
    }
}

impl<S, E> Responder for Sse<S>
where
    S: Stream<Item = Result<Event, E>> + 'static,
    E: Into<BoxError>,
{
    type Body = BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok()
            .content_type(russe::MEDIA_TYPE_STR)
            .insert_header(ContentEncoding::Identity)
            .insert_header(CacheControl(vec![CacheDirective::NoCache]))
            .body(self)
    }
}

impl<S, E> MessageBody for Sse<S>
where
    S: Stream<Item = Result<Event, E>>,
    E: Into<BoxError>,
{
    type Error = BoxError;

    fn size(&self) -> BodySize {
        BodySize::Stream
    }

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Bytes, Self::Error>>> {
        let this = self.project();

        if let Some(retry) = this.retry_interval.take() {
            cx.waker().wake_by_ref();
            return Poll::Ready(Some(
                russe::Event::Retry(retry)
                    .into_bytestring()
                    .map(ByteString::into_bytes)
                    .map_err(Into::into),
            ));
        }

        if let Poll::Ready(msg) = this.stream.poll_next(cx) {
            return match msg {
                Some(Ok(msg)) => Poll::Ready(Some(msg.into_bytes())),
                Some(Err(err)) => Poll::Ready(Some(Err(err.into()))),
                None => Poll::Ready(None),
            };
        }

        if let Some(keep_alive) = this.keep_alive
            && keep_alive.poll_tick(cx).is_ready()
        {
            return Poll::Ready(Some(
                russe::Event::Comment("keep-alive".into())
                    .into_bytestring()
                    .map(ByteString::into_bytes)
                    .map_err(Into::into),
            ));
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use actix_web::{body, test::TestRequest};
    use futures_util::{FutureExt as _, StreamExt as _, future::poll_fn, stream, task::noop_waker};
    use indoc::indoc;
    use tokio::time::sleep;

    use super::*;
    use crate::Data;

    #[actix_web::test]
    async fn data_builders_and_setters_encode_fields() {
        let mut data = Data::new("old data").id("old ID").event("old event");

        data.set_data("payload");
        data = data.id("42");
        data.set_event("update");

        let json = Data::new(serde_json::json!({ "bar": 42 }).to_string());
        let events = stream::iter([Event::Data(data), json.into()]);
        let sse = Sse::from_infallible_stream(events);

        assert_eq!(
            body::to_bytes(sse).await.unwrap(),
            "id: 42\nevent: update\ndata: payload\n\ndata: {\"bar\":42}\n\n",
        );
    }

    #[actix_web::test]
    async fn message_fields_are_encoded() {
        let events = stream::iter([
            Event::Comment("foo".into()),
            Data::new("\n").into(),
            Data::new("foo").id("42").event("bar").into(),
        ]);
        let sse = Sse::from_infallible_stream(events);

        assert_eq!(
            body::to_bytes(sse).await.unwrap(),
            ": foo\n\ndata: \ndata: \n\nid: 42\nevent: bar\ndata: foo\n\n",
        );
    }

    #[actix_web::test]
    async fn message_retry_is_omitted_by_default() {
        let events = stream::iter([
            Data::new("payload").into(),
            Data::new(serde_json::json!({ "bar": 42 }).to_string()).into(),
        ]);
        let sse = Sse::from_infallible_stream(events);

        assert_eq!(
            body::to_bytes(sse).await.unwrap(),
            indoc! {r#"
                data: payload

                data: {"bar":42}

            "#},
        );
    }

    #[actix_web::test]
    async fn message_retry_can_be_zero() {
        let data = Data::new("payload")
            .id("42")
            .event("update")
            .retry(Duration::ZERO);
        let sse = Sse::from_infallible_stream(stream::iter([data.into()]));

        assert_eq!(
            body::to_bytes(sse).await.unwrap(),
            indoc! {"
                id: 42
                event: update
                retry: 0
                data: payload

            "},
        );
    }

    #[actix_web::test]
    async fn consecutive_messages_have_independent_retry_fields() {
        let mut data =
            Data::new(serde_json::json!({ "bar": 42 }).to_string()).retry(Duration::from_secs(1));

        data.set_retry(Duration::from_millis(2345));

        let events = stream::iter([
            Data::new("first").retry(Duration::from_secs(1)).into(),
            data.into(),
            Data::new("third").into(),
        ]);
        let sse = Sse::from_infallible_stream(events);

        assert_eq!(
            body::to_bytes(sse).await.unwrap(),
            indoc! {r#"
                retry: 1000
                data: first

                retry: 2345
                data: {"bar":42}

                data: third

            "#},
        );
    }

    #[actix_web::test]
    async fn initial_retry_precedes_message_retry() {
        let events = stream::iter([Data::new("payload")
            .retry(Duration::from_millis(1234))
            .into()]);
        let mut sse =
            Sse::from_infallible_stream(events).with_retry_duration(Duration::from_millis(42));

        let initial = poll_fn(|cx| Pin::new(&mut sse).poll_next(cx)).await;
        let message = poll_fn(|cx| Pin::new(&mut sse).poll_next(cx)).await;
        let end = poll_fn(|cx| Pin::new(&mut sse).poll_next(cx)).await;

        assert_eq!(
            initial.unwrap().unwrap(),
            indoc! {"
                retry: 42

            "},
        );
        assert_eq!(
            message.unwrap().unwrap(),
            indoc! {"
                retry: 1234
                data: payload

            "},
        );
        assert!(end.is_none());
    }

    #[actix_web::test]
    async fn sse_from_external_streams() {
        let st = stream::empty::<Result<_, Infallible>>();
        let sse = Sse::from_stream(st);
        assert_eq!(body::to_bytes(sse).await.unwrap(), "");

        let st = stream::once(async { Ok::<_, Infallible>(Event::Data(Data::new("foo"))) });
        let sse = Sse::from_stream(st);
        assert_eq!(body::to_bytes(sse).await.unwrap(), "data: foo\n\n");

        let st = stream::repeat(Ok::<_, Infallible>(Event::Data(Data::new("foo")))).take(2);
        let sse = Sse::from_stream(st);
        assert_eq!(
            body::to_bytes(sse).await.unwrap(),
            "data: foo\n\ndata: foo\n\n",
        );
    }

    #[actix_web::test]
    async fn normalizes_line_endings() {
        let events = stream::iter([
            Event::Data(Data::new("first\r\nsecond\rthird\n")),
            Event::Comment("first\r\nsecond\rthird\n".into()),
        ]);
        let sse = Sse::from_infallible_stream(events);

        assert_eq!(
            body::to_bytes(sse).await.unwrap(),
            "data: first\ndata: second\ndata: third\ndata: \n\n: first\n: second\n: third\n: \n\n",
        );
    }

    #[actix_web::test]
    async fn rejects_invalid_message_fields() {
        for (id, event) in [
            ("bad\nid", "update"),
            ("bad\rid", "update"),
            ("bad\0id", "update"),
            ("42", "bad\nevent"),
            ("42", "bad\revent"),
        ] {
            let events = stream::iter([Event::Data(Data::new("payload").id(id).event(event))]);
            let sse = Sse::from_infallible_stream(events);

            let err = body::to_bytes(sse).await.unwrap_err();

            assert!(matches!(
                err.downcast_ref::<russe::EncodeError>(),
                Some(russe::EncodeError::InvalidFieldValue { .. }),
            ));
        }
    }

    #[test]
    fn retry_is_first_msg() {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        let mut sse = Sse::from_stream(InfallibleStream::new(tokio_stream::empty()))
            .with_retry_duration(Duration::from_millis(42));
        match Pin::new(&mut sse).poll_next(&mut cx) {
            Poll::Ready(Some(Ok(bytes))) => assert_eq!(bytes, "retry: 42\n\n"),
            res => panic!("poll should return retry message, got {res:?}"),
        }
    }

    #[actix_web::test]
    async fn retry_precedes_stream_events() {
        let events = stream::iter([Event::Comment("first".into())]);
        let sse =
            Sse::from_infallible_stream(events).with_retry_duration(Duration::from_millis(42));

        assert_eq!(
            body::to_bytes(sse).await.unwrap(),
            "retry: 42\n\n: first\n\n",
        );
    }

    #[actix_web::test]
    async fn receiver_errors_are_preserved() {
        let (sender, receiver) = mpsc::channel(1);
        let sse = Sse::from_stream(ReceiverStream::new(receiver));

        sender
            .send(Err(std::io::Error::other("stream failed")))
            .await
            .unwrap();

        let err = body::to_bytes(sse).await.unwrap_err();

        assert_eq!(
            err.downcast_ref::<std::io::Error>().unwrap().to_string(),
            "stream failed",
        );
    }

    #[actix_web::test]
    async fn closed_receiver_ends_with_keep_alive() {
        let (sender, receiver) = mpsc::channel(1);
        let sse = Sse::from_infallible_receiver(receiver).with_keep_alive(Duration::from_millis(4));

        drop(sender);

        assert_eq!(body::to_bytes(sse).await.unwrap(), "");
    }

    #[actix_web::test]
    async fn appropriate_headers_are_set_on_responder() {
        let st = stream::empty::<Result<_, Infallible>>();
        let sse = Sse::from_stream(st);

        let res = sse.respond_to(&TestRequest::default().to_http_request());

        assert_eq!(res.status(), actix_web::http::StatusCode::OK);
        assert_eq!(
            res.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
        assert_eq!(res.headers().get("content-encoding").unwrap(), "identity");
        assert_eq!(res.headers().get("cache-control").unwrap(), "no-cache");
    }

    #[actix_web::test]
    async fn messages_are_received_from_sender() {
        let (sender, receiver) = tokio::sync::mpsc::channel(2);
        let mut sse = Sse::from_infallible_receiver(receiver);

        assert!(
            poll_fn(|cx| Pin::new(&mut sse).poll_next(cx))
                .now_or_never()
                .is_none()
        );

        sender
            .send(Event::Data(Data::new("bar").event("foo")))
            .await
            .unwrap();

        match poll_fn(|cx| Pin::new(&mut sse).poll_next(cx)).now_or_never() {
            Some(Some(Ok(bytes))) => assert_eq!(bytes, "event: foo\ndata: bar\n\n"),
            res => panic!("poll should return data message, got {res:?}"),
        }
    }

    #[actix_web::test]
    async fn keep_alive_is_sent() {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        let (sender, receiver) = tokio::sync::mpsc::channel(2);
        let mut sse =
            Sse::from_infallible_receiver(receiver).with_keep_alive(Duration::from_millis(4));

        assert!(Pin::new(&mut sse).poll_next(&mut cx).is_pending());

        sleep(Duration::from_millis(20)).await;

        match Pin::new(&mut sse).poll_next(&mut cx) {
            Poll::Ready(Some(Ok(bytes))) => assert_eq!(bytes, ": keep-alive\n\n"),
            res => panic!("poll should return keep-alive message, got {res:?}"),
        }

        assert!(Pin::new(&mut sse).poll_next(&mut cx).is_pending());

        sender.send(Event::Data(Data::new("foo"))).await.unwrap();

        match Pin::new(&mut sse).poll_next(&mut cx) {
            Poll::Ready(Some(Ok(bytes))) => assert_eq!(bytes, "data: foo\n\n"),
            res => panic!("poll should return data message, got {res:?}"),
        }
    }
}
