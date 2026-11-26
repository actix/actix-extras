//! Connection manager and fluent API for `reqwest` v0.13.
//!
//! This module provides two ways to interact with an SSE stream.
//!
//! 1. The [extension trait](ReqwestExt) to provide a fluent API to chain on a response
//!    (`.sse_stream()` in place of `.bytes_stream()`).
//! 2. The [connection manager](Manager) which takes ownership of the client and tracks
//!    `Last-Event-ID` header state.

use std::io;

use bytestring::ByteString;
use futures_util::{StreamExt as _, TryStreamExt as _, stream::BoxStream};
use reqwest_0_13::{
    Client, Request, Response,
    header::{self, HeaderValue},
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    task::JoinHandle,
};
use tokio_util::{codec::FramedRead, io::StreamReader};

use crate::{Decoder, Error, Event};

mod sealed {
    use super::*;

    pub trait Sealed {}
    impl Sealed for Response {}
}

/// SSE extension methods for `reqwest` v0.13.
pub trait ReqwestExt: sealed::Sealed {
    /// Returns a stream of server-sent events.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use reqwest_0_13 as reqwest;
    /// use futures_util::TryStreamExt as _;
    /// use russe::reqwest_0_13::ReqwestExt as _;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut events = reqwest::get("https://httpbin.org/sse")
    ///     .await?
    ///     .sse_stream();
    ///
    /// while let Some(event) = events.try_next().await? {
    ///     println!("{:?}", event);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn sse_stream(self) -> BoxStream<'static, Result<Event, Error>>;
}

impl ReqwestExt for Response {
    fn sse_stream(self) -> BoxStream<'static, Result<Event, Error>> {
        let body_stream = self.bytes_stream().map_err(io::Error::other);
        let body_reader = StreamReader::new(body_stream);

        let frame_reader = FramedRead::new(body_reader, Decoder::default());

        Box::pin(frame_reader)
    }
}

/// Connection manager that delivers server-sent events via a channel and tracks
/// `Last-Event-ID` header state.
///
/// Automatic retries and `Last-Event-ID` sending are coming soon.
///
/// # Examples
///
/// ```no_run
/// # use reqwest_0_13 as reqwest;
/// use russe::{Event, reqwest_0_13::Manager};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = reqwest::Client::new();
/// let request = client.get("https://httpbin.org/sse").build()?;
///
/// let mut manager = Manager::new(&client, request);
/// let (_task, mut events) = manager.send().await?;
///
/// while let Some(Ok(event)) = events.recv().await {
///     if let Event::Message(msg) = event {
///         println!("{}", msg.data);
///
///         if let Some(id) = msg.id {
///             manager.commit_id(id);
///         }
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
    last_event_id: Option<ByteString>,
    tx: UnboundedSender<Result<Event, Error>>,
    rx: Option<UnboundedReceiver<Result<Event, Error>>>,
}

impl Manager {
    /// Constructs a new SSE request manager.
    ///
    /// Sets `Accept: text/event-stream` if the request has no `Accept` header.
    ///
    /// # Panics
    ///
    /// Panics if the request has a stream body.
    pub fn new(client: &Client, req: Request) -> Self {
        const DEFAULT_ACCEPT: HeaderValue = HeaderValue::from_static(crate::MEDIA_TYPE_STR);

        let (tx, rx) = unbounded_channel();

        let mut req = req.try_clone().expect("Request should be clone-able");
        req.headers_mut()
            .entry(header::ACCEPT)
            .or_insert(DEFAULT_ACCEPT);

        Self {
            client: client.clone(),
            req,
            last_event_id: None,
            tx,
            rx: Some(rx),
        }
    }

    /// Sends the request and returns a task handle and a receiver of events.
    ///
    /// Waits for response headers before starting the task that reads the event stream. A failed
    /// request can be retried by calling this method again.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Http`] if the initial request fails. Body and decoding errors are sent
    /// through the event receiver.
    ///
    /// # Panics
    ///
    /// Panics if called after a successful send.
    pub async fn send(
        &mut self,
    ) -> Result<(JoinHandle<()>, UnboundedReceiver<Result<Event, Error>>), Error> {
        assert!(self.rx.is_some(), "The request has already been sent");

        let req = self.req.try_clone().unwrap();
        let res = self
            .client
            .execute(req)
            .await
            .map_err(|source| Error::Http {
                source: Box::new(source),
            })?;

        let tx = self.tx.clone();
        let rx = self.rx.take().unwrap();

        let task_handle = tokio::spawn(async move {
            let mut stream = res.sse_stream();

            while let Some(ev) = stream.next().await {
                let _ = tx.send(ev);
            }
        });

        Ok((task_handle, rx))
    }

    /// Stores the latest event ID for this manager.
    ///
    /// The stored ID is reserved for reconnect support.
    pub fn commit_id(&mut self, id: impl Into<ByteString>) {
        self.last_event_id = Some(id.into());
    }
}
