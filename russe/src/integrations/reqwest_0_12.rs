//! Connection manager and fluent API for `reqwest` v0.12.
//!
//! This module provides two ways to interact with an SSE stream.
//!
//! 1. The [extension trait](ReqwestExt) to provide a fluent API to chain on a response
//!    (`.sse_stream()` in place of `.bytes_stream()`).
//! 2. The [connection manager](Manager) which takes ownership of the client and tracks
//!    `Last-Event-ID` header state.

use std::io;

use bytestring::ByteString;
use futures_util::{TryStreamExt as _, stream::BoxStream};
use reqwest_0_12::{
    Client, Request, Response,
    header::{self, HeaderValue},
};
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, unbounded_channel},
        watch,
    },
    task::JoinHandle,
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
        source: reqwest_0_12::Error,
    },

    /// The committed event ID is invalid in an HTTP header.
    InvalidEventId {
        /// Source of the header error.
        source: header::InvalidHeaderValue,
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
    InvalidEventId { .. } => "Invalid Last-Event-ID header",
    Decode { source } => "{source}",
}

impl_more::impl_error_enum! {
    ManagerError:
    Http { source } => source,
    InvalidEventId { source } => source,
    Decode { source } => source,
}

impl_more::impl_enum_from!(DecodeError => ManagerError::Decode { source });

mod sealed {
    use super::*;

    pub trait Sealed {}
    impl Sealed for Response {}
}

/// SSE extension methods for `reqwest` v0.12.
pub trait ReqwestExt: sealed::Sealed {
    /// Returns a stream of server-sent events.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use reqwest_0_12 as reqwest;
    /// use futures_util::StreamExt as _;
    /// use russe::reqwest_0_12::ReqwestExt as _;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut events = reqwest::get("https://httpbin.org/sse")
    ///     .await?
    ///     .sse_stream();
    ///
    /// while let Some(Ok(event)) = events.next().await {
    ///     println!("{:?}", event);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn sse_stream(self) -> BoxStream<'static, Result<Event, DecodeError>>;
}

impl ReqwestExt for Response {
    fn sse_stream(self) -> BoxStream<'static, Result<Event, DecodeError>> {
        let body_stream = self.bytes_stream().map_err(io::Error::other);
        let body_reader = StreamReader::new(body_stream);

        let frame_reader = FramedRead::new(body_reader, Decoder::default());

        Box::pin(frame_reader)
    }
}

/// Connection manager that delivers server-sent events via a channel and tracks
/// `Last-Event-ID` header state.
///
/// Use [`Self::reconnect`] to reconnect when a response stream ends.
///
/// # Examples
///
/// ```no_run
/// # use reqwest_0_12 as reqwest;
/// # use std::time::Duration;
/// use russe::{Event, Reconnect, reqwest_0_12::Manager};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = reqwest::Client::new();
/// let req = client.get("https://httpbin.org/sse").build()?;
///
/// let mut manager = Manager::new(&client, req)
///     .reconnect(Reconnect::OnError {
///         delay: Duration::from_secs(1),
///         max_retries: 3,
///     });
///
/// let (_task, mut events) = manager.send().await?;
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
///
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Manager {
    client: Client,
    req: Request,
    last_event_id: watch::Sender<Option<ByteString>>,
    reconnect: Reconnect,
    sent: bool,
}

impl Manager {
    /// Constructs a new SSE request manager.
    ///
    /// Sets `Accept: text/event-stream` if the request has no `Accept` header.
    ///
    /// # Panics
    ///
    /// Panics if the request has a stream body.
    #[must_use]
    pub fn new(client: &Client, req: Request) -> Self {
        const DEFAULT_ACCEPT: HeaderValue = HeaderValue::from_static(crate::MEDIA_TYPE_STR);

        let mut req = req.try_clone().expect("Request should be clone-able");
        req.headers_mut()
            .entry(header::ACCEPT)
            .or_insert(DEFAULT_ACCEPT);

        Self {
            client: client.clone(),
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
    /// Returns [`ManagerError::InvalidEventId`] if the committed ID is invalid in a header, or
    /// [`ManagerError::Http`] if the initial request fails. The receiver reports later errors.
    ///
    /// # Panics
    ///
    /// Panics if called after a successful send.
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

        let client = self.client.clone();
        let req = self.req.try_clone().unwrap();

        let connect = async move |id: Option<ByteString>| {
            let mut req = req.try_clone().unwrap();

            if let Some(id) = id {
                let value = HeaderValue::from_bytes(id.as_bytes())
                    .map_err(|source| ManagerError::InvalidEventId { source })?;

                req.headers_mut().insert("last-event-id", value);
            }

            let res = client
                .execute(req)
                .await
                .map_err(|source| ManagerError::Http { source })?;

            Ok((res.status().as_u16() != 204).then(|| res.sse_stream()))
        };

        let id = self.last_event_id.borrow().clone();
        let first = connect(id).await?;
        self.sent = true;

        let (tx, rx) = unbounded_channel();

        let task_handle = tokio::spawn(crate::reconnect::run(
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

// - optionally read id from stream and set automatically
