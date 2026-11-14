use bytestring::ByteString;

/// Server-sent events data message containing a `data` field and optional `id` and `event` fields.
#[must_use]
#[derive(Debug, Clone)]
pub struct Data {
    pub(crate) id: Option<ByteString>,
    pub(crate) event: Option<ByteString>,
    pub(crate) data: ByteString,
}

impl Data {
    /// Constructs a new SSE data message with just the `data` field.
    ///
    /// # Examples
    /// ```
    /// let event = actix_sse::Event::Data(actix_sse::Data::new("foo"));
    /// ```
    pub fn new<T: Into<ByteString>>(data: T) -> Self {
        Self {
            id: None,
            event: None,
            data: data.into(),
        }
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
}
