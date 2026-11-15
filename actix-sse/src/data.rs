use std::time::Duration;

use bytestring::ByteString;

/// Server-sent events data message containing a `data` field and optional `id`, `event`, and `retry`
/// fields.
///
/// # Examples
/// ```
/// # #[actix_web::main] async fn test() {
/// use std::convert::Infallible;
///
/// use actix_web::body;
/// use actix_sse as sse;
/// use futures_util::stream;
/// use serde::Serialize;
///
/// #[derive(serde::Serialize)]
/// struct Foo {
///     bar: u32,
/// }
///
/// let sse = sse::Sse::from_stream(stream::iter([
///     Ok::<_, Infallible>(sse::Event::Data(sse::Data::new("foo"))),
///     Ok::<_, Infallible>(sse::Event::Data(
///         sse::Data::new(serde_json::to_string(&Foo { bar: 42 }).unwrap()),
///     )),
/// ]));
///
/// assert_eq!(
///     body::to_bytes(sse).await.unwrap(),
///     "data: foo\n\ndata: {\"bar\":42}\n\n",
/// );
/// # }; test();
/// ```
#[must_use]
#[derive(Debug, Clone)]
pub struct Data {
    pub(crate) id: Option<ByteString>,
    pub(crate) event: Option<ByteString>,
    pub(crate) retry: Option<Duration>,
    pub(crate) data: ByteString,
}

impl Data {
    /// Constructs a new SSE data message with just the `data` field.
    ///
    /// # Examples
    /// ```
    /// use actix_sse as sse;
    /// let event = sse::Event::Data(sse::Data::new("foo"));
    /// ```
    pub fn new(data: impl Into<ByteString>) -> Self {
        Self {
            id: None,
            event: None,
            retry: None,
            data: data.into(),
        }
    }

    /// Sets `data` field.
    pub fn set_data(&mut self, data: impl Into<ByteString>) {
        self.data = data.into();
    }

    /// Sets `id` field, returning a new data message.
    pub fn id(mut self, id: impl Into<ByteString>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets `event` name field, returning a new data message.
    pub fn event(mut self, event: impl Into<ByteString>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Sets `event` name field.
    pub fn set_event(&mut self, event: impl Into<ByteString>) {
        self.event = Some(event.into());
    }

    /// Sets `retry` field, returning a new data message.
    ///
    /// This field updates the connection's reconnection delay in milliseconds. The delay remains
    /// effective until another `retry` field replaces it. It does not retry individual message
    /// delivery.
    ///
    /// See the [SSE parsing rules](https://html.spec.whatwg.org/multipage/server-sent-events.html#interpreting-an-event-stream).
    pub fn retry(mut self, retry: Duration) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Sets `retry` field.
    ///
    /// See [`Self::retry`] for how this field updates the connection's reconnection delay.
    pub fn set_retry(&mut self, retry: Duration) {
        self.retry = Some(retry);
    }
}
