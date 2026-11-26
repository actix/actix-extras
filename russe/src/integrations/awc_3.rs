//! Connection manager and fluent API for AWC 3.
//!
//! This module provides two ways to interact with an SSE stream.
//!
//! 1. The [extension trait](AwcExt) provides a fluent API to chain `.sse_stream()` on a response.
//! 2. The [connection manager](Manager) freezes the request and tracks
//!    `Last-Event-ID` header state.

use std::{fmt, io};

use awc_3::{
    ClientRequest, ClientResponse, FrozenClientRequest, error::PayloadError, http::header,
};
use bytes::Bytes;
use bytestring::ByteString;
use futures_util::{Stream, TryStreamExt as _, stream::LocalBoxStream};
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, unbounded_channel},
        watch,
    },
    task::{JoinHandle, spawn_local},
};
use tokio_util::{codec::FramedRead, io::StreamReader};

use crate::{DecodeError, Decoder, Event, Reconnect};

/// Error returned by this client's connection manager.
#[derive(Debug)]
#[non_exhaustive]
pub enum ManagerError {
    /// The HTTP request failed.
    Http {
        /// Source of the request error.
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Reading or decoding the response body failed.
    Decode {
        /// Source of the stream error.
        source: DecodeError,
    },
}

impl_more::impl_display_enum! {
    ManagerError:
    Http { .. } => "HTTP request failed",
    Decode { source } => "{source}",
}

impl_more::impl_error_enum! {
    ManagerError:
    Http { source } => source.as_ref(),
    Decode { source } => source,
}

mod sealed {
    use super::*;

    pub trait Sealed {}
    impl<S> Sealed for ClientResponse<S> {}
}

/// SSE extension methods for AWC 3.
pub trait AwcExt: sealed::Sealed {
    /// Returns a local stream of server-sent events.
    ///
    /// The stream is not `Send` because AWC responses are not `Send`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures_util::StreamExt as _;
    /// use russe::awc_3::AwcExt as _;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut events = awc_3::Client::default()
    ///     .get("http://httpbin.org/sse")
    ///     .send()
    ///     .await?
    ///     .sse_stream();
    ///
    /// while let Some(Ok(event)) = events.next().await {
    ///     println!("{:?}", event);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn sse_stream(self) -> LocalBoxStream<'static, Result<Event, DecodeError>>;
}

impl<S> AwcExt for ClientResponse<S>
where
    S: Stream<Item = Result<Bytes, PayloadError>> + Unpin + 'static,
{
    fn sse_stream(self) -> LocalBoxStream<'static, Result<Event, DecodeError>> {
        let body_stream = self.map_err(io::Error::other);
        let body_reader = StreamReader::new(body_stream);

        let frame_reader = FramedRead::new(body_reader, Decoder::default());

        Box::pin(frame_reader)
    }
}

/// Connection manager that delivers server-sent events through a channel and tracks
/// the last event ID.
///
/// Runs on an Actix runtime, a Tokio local runtime, or a Tokio [`LocalSet`](tokio::task::LocalSet).
/// Use [`Self::reconnect`] to reconnect when a response stream ends.
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// use russe::{Event, Reconnect, awc_3::Manager};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = awc_3::Client::default();
/// let req = client.get("http://httpbin.org/sse");
///
/// let mut manager = Manager::new(req)
///     .reconnect(Reconnect::OnError {
///         delay: Duration::from_secs(1),
///         max_retries: 3,
///     });
///
/// let (task, mut events) = manager.send().await?;
///
/// while let Some(event) = events.recv().await {
///     match event {
///         Ok(Event::Message(msg)) => {
///             println!("{}", msg.data);
///
///             if let Some(id) = msg.id {
///                 manager.commit_id(id);
///             }
///         }
///         Ok(_) => {}
///         Err(err) => eprintln!("{err}"),
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct Manager {
    req: FrozenClientRequest,
    last_event_id: watch::Sender<Option<ByteString>>,
    reconnect: Reconnect,
    sent: bool,
}

impl fmt::Debug for Manager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            req,
            last_event_id,
            reconnect,
            sent,
        } = self;

        f.debug_struct("Manager")
            .field("method", req.get_method())
            .field("uri", req.get_uri())
            .field("last_event_id", last_event_id)
            .field("reconnect", reconnect)
            .field("sent", sent)
            .finish()
    }
}

fn http_error(err: impl std::error::Error) -> ManagerError {
    // AWC request errors can contain sources that are not Send or Sync.
    ManagerError::Http {
        source: err_report::Report::new(err).to_string().into(),
    }
}

impl Manager {
    /// Freezes the request and constructs an SSE request manager.
    ///
    /// Sends an empty body and sets `Accept: text/event-stream` if the request has no
    /// `Accept` header.
    ///
    /// # Panics
    ///
    /// Panics if the request cannot be frozen.
    #[must_use]
    pub fn new(req: ClientRequest) -> Self {
        let req = req
            .insert_header_if_none((header::ACCEPT, crate::MEDIA_TYPE_STR))
            .freeze()
            .expect("Failed to freeze AWC request");

        Self {
            req,
            last_event_id: watch::Sender::new(None),
            reconnect: Reconnect::default(),
            sent: false,
        }
    }

    /// Selects the reconnect behavior for the request.
    ///
    /// Default is [`Reconnect::Never`].
    #[must_use]
    pub fn reconnect(mut self, policy: Reconnect) -> Self {
        self.reconnect = policy;
        self
    }

    /// Sends the request and returns a task handle and a receiver of events.
    ///
    /// Waits for response headers before starting the task that reads the event stream. A failed
    /// request can be retried by calling this method again. By default, the event channel closes at EOF.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerError::Http`] if the initial request fails. Body and decoding errors are sent
    /// through the event receiver.
    ///
    /// # Panics
    ///
    /// Panics if called after a successful send or from a context that does not support
    /// [`spawn_local`].
    pub async fn send(
        &mut self,
    ) -> Result<
        (
            JoinHandle<()>,
            UnboundedReceiver<Result<Event, ManagerError>>,
        ),
        ManagerError,
    > {
        assert!(!self.sent, "The request has already been sent");

        let req = self.req.clone();

        let connect = async move |id: Option<ByteString>| {
            let req = match id {
                Some(id) => req.extra_header(("last-event-id", id.as_str())).send(),
                None => req.send(),
            };

            let res = req.await.map_err(http_error)?;

            Ok((res.status().as_u16() != 204).then(|| {
                res.sse_stream()
                    .map_err(|source| ManagerError::Decode { source })
            }))
        };

        let id = self.last_event_id.borrow().clone();
        let first = connect(id).await?;
        self.sent = true;

        let (tx, rx) = unbounded_channel();

        let task_handle = spawn_local(crate::reconnect::run(
            first,
            self.reconnect,
            self.last_event_id.subscribe(),
            tx,
            connect,
        ));

        Ok((task_handle, rx))
    }

    /// Stores the latest event ID for this manager.
    ///
    /// The initial request and each reconnect send the latest committed ID in the `Last-Event-ID`
    /// header. An empty ID leaves the original request headers in effect.
    pub fn commit_id(&mut self, id: impl Into<ByteString>) {
        let id = id.into();
        self.last_event_id
            .send_replace((!id.is_empty()).then_some(id));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use actix_http::{BoxedPayloadStream, Payload};
    use awc_3::{Client, error::PayloadError, test::TestResponse};
    use bytes::Bytes;
    use futures_test::assert_stream_pending;
    use futures_util::{StreamExt as _, TryStreamExt as _, stream};
    use indoc::indoc;
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tokio_util::time::FutureExt as _;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::{AwcExt as _, Manager, ManagerError};
    use crate::{DecodeError, Event, Message};

    async fn sse_server() -> MockServer {
        let server = MockServer::start().await;
        let body = indoc! {"
            id: 42
            event: update
            data: hello

            : heartbeat

        "};

        Mock::given(method("GET"))
            .and(path("/events"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, crate::MEDIA_TYPE_STR))
            .expect(1)
            .mount(&server)
            .await;

        server
    }

    #[tokio::test(flavor = "local")]
    async fn reconnect_uses_committed_id_and_server_delay() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw("retry: 1\n\ndata: hello\n\n", crate::MEDIA_TYPE_STR),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(wiremock::matchers::header("last-event-id", "42"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let req = Client::default().get(server.uri());

        let mut manager = Manager::new(req).reconnect(crate::Reconnect::OnEof {
            delay: Duration::from_secs(60),
            max_retries: 3,
        });
        manager.commit_id("42");

        let (task, mut events) = manager.send().await.unwrap();
        task.timeout(Duration::from_secs(5)).await.unwrap().unwrap();

        assert!(matches!(events.recv().await, Some(Ok(Event::Retry(_)))));
        assert!(matches!(events.recv().await, Some(Ok(Event::Message(_)))));
        assert!(events.recv().await.is_none());

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].headers.get("last-event-id").unwrap(), "42");
    }

    #[tokio::test(flavor = "local")]
    async fn response_stream_decodes_sse_events() {
        let server = sse_server().await;
        let url = format!("{}/events", server.uri());
        let client = Client::builder().timeout(Duration::from_secs(5)).finish();

        let events = client
            .get(url)
            .send()
            .await
            .unwrap()
            .sse_stream()
            .try_collect::<Vec<_>>()
            .await
            .unwrap();

        assert_eq!(
            events,
            vec![
                Event::Message(Message {
                    data: "hello".into(),
                    event: Some("update".into()),
                    retry: None,
                    id: Some("42".into()),
                }),
                Event::Comment("heartbeat".into()),
            ],
        );
    }

    #[tokio::test]
    async fn response_stream_decodes_custom_payload() {
        let res =
            TestResponse::default()
                .finish()
                .map_body(|_, _| -> Payload<BoxedPayloadStream> {
                    Bytes::from_static(b"data: hello\n\n").into()
                });

        let events = res.sse_stream().try_collect::<Vec<_>>().await.unwrap();

        assert_eq!(
            events,
            vec![Event::Message(Message {
                data: "hello".into(),
                event: None,
                retry: None,
                id: None,
            })],
        );
    }

    #[tokio::test]
    async fn response_stream_decodes_events_before_eof() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let res = TestResponse::default()
            .finish()
            .map_body(|_, _| UnboundedReceiverStream::new(rx).boxed_local().into());
        let mut events = res.sse_stream();

        tx.send(Ok(Bytes::from_static(b"data: hel"))).unwrap();

        assert_stream_pending!(events);

        tx.send(Ok(Bytes::from_static(b"lo\n\n"))).unwrap();

        let event = events
            .try_next()
            .timeout(Duration::from_secs(5))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            event,
            Some(Event::Message(Message {
                data: "hello".into(),
                event: None,
                retry: None,
                id: None,
            })),
        );

        drop(tx);

        assert!(events.try_next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn response_stream_reports_payload_errors() {
        let res =
            TestResponse::default()
                .finish()
                .map_body(|_, _| -> Payload<BoxedPayloadStream> {
                    stream::once(async { Err(PayloadError::Incomplete(None)) })
                        .boxed_local()
                        .into()
                });

        let error = res.sse_stream().try_next().await.unwrap_err();

        let DecodeError::Io { source } = error else {
            panic!("Expected an I/O error; got: {error:?}");
        };

        assert!(matches!(
            source.get_ref().unwrap().downcast_ref::<PayloadError>(),
            Some(PayloadError::Incomplete(None)),
        ));
    }

    #[tokio::test]
    async fn response_stream_reports_invalid_utf8() {
        let res = TestResponse::default()
            .set_payload(Bytes::from_static(b"data: \xff\n\n"))
            .finish();

        let error = res.sse_stream().try_next().await.unwrap_err();

        assert!(matches!(error, DecodeError::InvalidUtf8 { .. }));
    }

    #[tokio::test(flavor = "local")]
    #[should_panic(expected = "Failed to freeze AWC request")]
    async fn manager_panics_on_invalid_request() {
        let req = Client::default().get("http://[");
        let _ = Manager::new(req);
    }

    #[tokio::test(flavor = "local")]
    async fn manager_delivers_sse_events() {
        let server = sse_server().await;
        let url = format!("{}/events", server.uri());

        let client = Client::builder().timeout(Duration::from_secs(5)).finish();
        let req = client.get(url);

        let mut manager = Manager::new(req);

        let (task, events) = manager.send().await.unwrap();

        let events = UnboundedReceiverStream::new(events)
            .try_collect::<Vec<_>>()
            .timeout(Duration::from_secs(5))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            events,
            vec![
                Event::Message(Message {
                    data: "hello".into(),
                    event: Some("update".into()),
                    retry: None,
                    id: Some("42".into()),
                }),
                Event::Comment("heartbeat".into()),
            ],
        );

        task.await.unwrap();
    }

    #[tokio::test(flavor = "local")]
    async fn manager_can_retry_after_initial_http_error() {
        let server = MockServer::start().await;
        let slow_response = Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
            .expect(1)
            .mount_as_scoped(&server)
            .await;

        let client = Client::builder().timeout(Duration::from_secs(1)).finish();
        let req = client.get(server.uri());

        let mut manager = Manager::new(req);

        assert!(matches!(
            manager.send().await.unwrap_err(),
            ManagerError::Http { .. }
        ));

        drop(slow_response);

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(": ping\n\n", crate::MEDIA_TYPE_STR),
            )
            .expect(1)
            .mount(&server)
            .await;

        let (task, mut events) = manager.send().await.unwrap();
        assert_eq!(
            events.recv().await.unwrap().unwrap(),
            Event::Comment("ping".into())
        );

        task.await.unwrap();

        assert!(events.is_closed());
        assert!(events.recv().await.is_none());
    }

    #[tokio::test(flavor = "local")]
    async fn manager_sends_decoding_errors_through_receiver() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(b"data: \xff\n\n", crate::MEDIA_TYPE_STR),
            )
            .expect(1)
            .mount(&server)
            .await;

        let req = Client::default().get(server.uri());

        let mut manager = Manager::new(req);

        let (task, mut events) = manager.send().await.unwrap();
        drop(manager);
        assert!(matches!(
            events.recv().await,
            Some(Err(ManagerError::Decode {
                source: DecodeError::InvalidUtf8 { .. }
            }))
        ));

        task.await.unwrap();
    }

    #[tokio::test(flavor = "local")]
    async fn manager_sends_accept_header() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = Client::default();
        let custom = "text/event-stream, text/plain;q=0.1";

        for req in [
            client.get(server.uri()),
            client.get(server.uri()).insert_header(("accept", custom)),
        ] {
            let mut manager = Manager::new(req);

            let (task, _) = manager.send().await.unwrap();
            task.await.unwrap();
        }

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].headers["accept"], crate::MEDIA_TYPE_STR);
        assert_eq!(requests[1].headers["accept"], custom);
    }
}
