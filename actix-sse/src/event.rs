use bytes::Bytes;
use bytestring::ByteString;

use crate::{BoxError, Data};

/// Server-sent events message containing one or more fields.
#[must_use]
#[derive(Debug, Clone)]
pub enum Event {
    /// A `data` message with optional ID, event name, and reconnection delay.
    ///
    /// Data messages looks like this in the response stream.
    /// ```plain
    /// event: foo
    /// id: 42
    /// data: my data
    ///
    /// data: {
    /// data:   "multiline": "data"
    /// data: }
    /// ```
    Data(Data),

    /// A comment message.
    ///
    /// Comments look like this in the response stream.
    /// ```plain
    /// : my comment
    ///
    /// : another comment
    /// ```
    Comment(ByteString),
}

impl Event {
    /// Encodes the event in event-stream format.
    pub(crate) fn into_bytes(self) -> Result<Bytes, BoxError> {
        let event = match self {
            Self::Data(Data {
                id,
                event,
                retry,
                data,
            }) => russe::Event::Message(russe::Message {
                data,
                event,
                id,
                retry,
            }),

            Self::Comment(comment) => russe::Event::Comment(comment),
        };

        event
            .into_bytestring()
            .map(ByteString::into_bytes)
            .map_err(Into::into)
    }
}

impl From<Data> for Event {
    fn from(data: Data) -> Self {
        Self::Data(data)
    }
}
