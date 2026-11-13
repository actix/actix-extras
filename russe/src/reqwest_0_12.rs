//! Utilities for `reqwest` v0.12.

use std::io;

use bytestring::ByteString;
use futures_util::{StreamExt as _, TryStreamExt as _, stream::BoxStream};
use reqwest_0_12::{Client, Request, Response};
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

/// SSE extension methods for `reqwest` v0.12.
pub trait ReqwestExt: sealed::Sealed {
    /// Returns a stream of server-sent events.
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

/// An SSE request manager which tracks latest IDs and automatically reconnects.
#[derive(Debug)]
pub struct Manager {
    client: Client,
    req: Request,
    last_event_id: Option<ByteString>,
    tx: UnboundedSender<Result<Event, Error>>,
    rx: Option<UnboundedReceiver<Result<Event, Error>>>,
}

impl Manager {
    /// Constructs new SSE request manager.
    ///
    /// No attempts are made to validate or modify the given request.
    ///
    /// # Panics
    ///
    /// Panics if request is given a stream body.
    pub fn new(client: &Client, req: Request) -> Self {
        let (tx, rx) = unbounded_channel();

        let req = req.try_clone().expect("Request should be clone-able");

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
    /// Waits for response headers before starting the task that reads the event stream.
    /// A failed request can be retried by calling this method again.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Http`] if the initial request fails.
    /// Body and decoding errors are sent through the event receiver.
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

    /// Commits an event ID for this manager.
    ///
    /// The given ID will be used as the `Last-Event-Id` header in case of reconnects.
    pub fn commit_id(&mut self, id: impl Into<ByteString>) {
        self.last_event_id = Some(id.into());
    }
}

// - optionally read id from stream and set automatically
