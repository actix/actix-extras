use std::time::Duration;

use bytes::{BufMut, Bytes, BytesMut};
use bytestring::ByteString;

use crate::Data;

/// Server-sent events message containing one or more fields.
#[must_use]
#[derive(Debug, Clone)]
pub enum Event {
    /// A `data` message with optional ID and event name.
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
    /// Splits data into lines and prepend each line with `prefix`.
    pub(crate) fn line_split_with_prefix(
        buf: &mut BytesMut,
        prefix: &'static str,
        data: ByteString,
    ) {
        // initial buffer size guess is len(data) + 10 lines of prefix + EOLs + EOF
        buf.reserve(data.len() + (10 * (prefix.len() + 1)) + 1);

        // append prefix + space + line to buffer
        for line in data.split('\n') {
            buf.put_slice(prefix.as_bytes());
            buf.put_slice(line.as_bytes());
            buf.put_u8(b'\n');
        }
    }

    /// Serializes message into event-stream format.
    pub(crate) fn into_bytes(self) -> Bytes {
        let mut buf = BytesMut::new();

        match self {
            Event::Data(Data { id, event, data }) => {
                if let Some(text) = id {
                    buf.put_slice(b"id: ");
                    buf.put_slice(text.as_bytes());
                    buf.put_u8(b'\n');
                }

                if let Some(text) = event {
                    buf.put_slice(b"event: ");
                    buf.put_slice(text.as_bytes());
                    buf.put_u8(b'\n');
                }

                Self::line_split_with_prefix(&mut buf, "data: ", data);
            }

            Event::Comment(text) => Self::line_split_with_prefix(&mut buf, ": ", text),
        }

        // final newline to mark end of message
        buf.put_u8(b'\n');

        buf.freeze()
    }

    /// Serializes retry message into event-stream format.
    pub(crate) fn retry_to_bytes(retry: Duration) -> Bytes {
        Bytes::from(format!("retry: {}\n\n", retry.as_millis()))
    }

    /// Serializes a keep-alive event-stream comment message into bytes.
    pub(crate) const fn keep_alive_bytes() -> Bytes {
        Bytes::from_static(b": keep-alive\n\n")
    }
}

impl From<Data> for Event {
    fn from(data: Data) -> Self {
        Self::Data(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_retry_message() {
        assert_eq!(
            Event::retry_to_bytes(Duration::from_millis(1)),
            "retry: 1\n\n",
        );
        assert_eq!(
            Event::retry_to_bytes(Duration::from_secs(10)),
            "retry: 10000\n\n",
        );
    }

    #[test]
    fn line_split_format() {
        let mut buf = BytesMut::new();
        Event::line_split_with_prefix(&mut buf, "data: ", ByteString::from("foo"));
        assert_eq!(buf, "data: foo\n");

        let mut buf = BytesMut::new();
        Event::line_split_with_prefix(&mut buf, "data: ", ByteString::from("foo\nbar"));
        assert_eq!(buf, "data: foo\ndata: bar\n");
    }

    #[test]
    fn into_bytes_format() {
        assert_eq!(Event::Comment("foo".into()).into_bytes(), ": foo\n\n");

        assert_eq!(
            Event::Data(Data {
                id: None,
                event: None,
                data: "foo".into()
            })
            .into_bytes(),
            "data: foo\n\n"
        );

        assert_eq!(
            Event::Data(Data {
                id: None,
                event: None,
                data: "\n".into()
            })
            .into_bytes(),
            "data: \ndata: \n\n"
        );

        assert_eq!(
            Event::Data(Data {
                id: Some("42".into()),
                event: None,
                data: "foo".into()
            })
            .into_bytes(),
            "id: 42\ndata: foo\n\n"
        );

        assert_eq!(
            Event::Data(Data {
                id: None,
                event: Some("bar".into()),
                data: "foo".into()
            })
            .into_bytes(),
            "event: bar\ndata: foo\n\n"
        );

        assert_eq!(
            Event::Data(Data {
                id: Some("42".into()),
                event: Some("bar".into()),
                data: "foo".into()
            })
            .into_bytes(),
            "id: 42\nevent: bar\ndata: foo\n\n"
        );
    }
}
