use bytes::BytesMut;

use crate::{Error, Event, EventRef};

/// SSE encoder for use with [`tokio_util::codec::FramedWrite`].
///
/// Implements [`tokio_util::codec::Encoder<Event>`]. Each event ends with a blank line.
/// Use [`Encoder::encode_borrowed`] to write borrowed field text directly.
/// CRLF and bare CR in message data and comments are converted to LF. Empty lines are preserved.
/// Retry delays are written in whole milliseconds. Fractions of a millisecond are discarded.
///
/// Returns [`Error::InvalidFieldValue`] if an event name contains CR or LF,
/// or an ID contains NUL, CR, or LF.
/// These fields are checked before any bytes are added to the output buffer.
///
/// # Examples
///
/// ```
/// use bytes::BytesMut;
/// use russe::{Encoder, Event};
/// use tokio_util::codec::Encoder as _;
///
/// let mut output = BytesMut::new();
/// Encoder::default().encode(Event::Comment("keep-alive".into()), &mut output)?;
/// assert_eq!(output, ": keep-alive\n\n");
/// # Ok::<_, russe::Error>(())
/// ```
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Encoder {}

impl tokio_util::codec::Encoder<Event> for Encoder {
    type Error = Error;

    fn encode(&mut self, item: Event, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.encode_into(dst)
    }
}

impl Encoder {
    /// Encodes an event without taking ownership of its field text.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidFieldValue`] if an ID or event name is invalid.
    /// Field validation occurs before any bytes are added to the output buffer.
    pub fn encode_borrowed(&mut self, item: EventRef<'_>, dst: &mut BytesMut) -> Result<(), Error> {
        item.encode_into(dst)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::BytesMut;
    use futures_util::SinkExt as _;
    use quickcheck_macros::quickcheck;
    use tokio_util::codec::{Decoder as _, Encoder as _, FramedWrite};

    use super::*;
    use crate::Message;

    #[quickcheck]
    fn message_data_roundtrips(data: String) {
        let expected = Message::data(data.replace("\r\n", "\n").replace('\r', "\n"));
        let mut output = BytesMut::new();

        Encoder::default()
            .encode(Event::Message(Message::data(data)), &mut output)
            .unwrap();

        assert_eq!(
            crate::Decoder::default().decode(&mut output).unwrap(),
            Some(Event::Message(expected)),
        );
        assert!(output.is_empty());
    }

    #[test]
    fn encodes_message_data() {
        let message = Message {
            data: "héllo".into(),
            event: None,
            retry: None,
            id: None,
        };
        let mut output = BytesMut::new();

        Encoder::default()
            .encode(Event::Message(message), &mut output)
            .unwrap();

        assert_eq!(output, "data: héllo\n\n");
    }

    #[test]
    fn encodes_borrowed_events() {
        for (event, expected) in [
            (
                EventRef::Message {
                    data: "é\r\n世界\r🦀\n",
                    event: Some("update\0name"),
                    retry: Some(Duration::from_micros(1_999)),
                    id: Some("42"),
                },
                "id: 42\nevent: update\0name\nretry: 1\ndata: é\ndata: 世界\ndata: 🦀\ndata: \n\n",
            ),
            (EventRef::Comment("é\r\n世界\r"), ": é\n: 世界\n: \n\n"),
            (
                EventRef::Retry(Duration::MAX),
                "retry: 18446744073709551615999\n\n",
            ),
        ] {
            let mut output = BytesMut::from("data: previous\n\n");

            Encoder::default()
                .encode_borrowed(event, &mut output)
                .unwrap();

            assert_eq!(&output[..16], b"data: previous\n\n");
            assert_eq!(&output[16..], expected.as_bytes());
            assert_eq!(event.into_bytestring().unwrap(), expected);
        }
    }

    #[test]
    fn rejects_invalid_borrowed_fields_without_changing_output() {
        for (id, event, field, character, offset) in [
            ("é\r\0id", "update", "id", '\r', 2),
            ("é\0\rid", "update", "id", '\0', 2),
            ("42", "世界\nevent", "event", '\n', 6),
        ] {
            let event = EventRef::Message {
                data: "payload",
                event: Some(event),
                retry: None,
                id: Some(id),
            };
            let mut output = BytesMut::from("data: previous\n\n");

            let Error::InvalidFieldValue {
                field: actual_field,
                character: actual_character,
                offset: actual_offset,
            } = Encoder::default()
                .encode_borrowed(event, &mut output)
                .unwrap_err()
            else {
                panic!("Expected an invalid field error");
            };

            assert_eq!(
                (actual_field, actual_character, actual_offset),
                (field, character, offset)
            );
            assert_eq!(output, "data: previous\n\n");
        }
    }

    #[test]
    fn encodes_empty_and_multiline_data() {
        for (data, expected) in [
            ("", "data: \n\n"),
            (
                "\nfirst\n\n last\n",
                "data: \ndata: first\ndata: \ndata:  last\ndata: \n\n",
            ),
            (
                "first\r\nsecond\rthird\n",
                "data: first\ndata: second\ndata: third\ndata: \n\n",
            ),
        ] {
            let message = Message {
                data: data.into(),
                event: None,
                retry: None,
                id: None,
            };
            let mut output = BytesMut::new();

            Encoder::default()
                .encode(Event::Message(message), &mut output)
                .unwrap();

            assert_eq!(output, expected, "input: {data:?}");
        }
    }

    #[test]
    fn encodes_message_fields() {
        let message = Message {
            data: "payload".into(),
            event: Some(" update".into()),
            retry: Some(Duration::from_micros(1_234_567)),
            id: Some("42".into()),
        };
        let mut output = BytesMut::new();

        Encoder::default()
            .encode(Event::Message(message), &mut output)
            .unwrap();

        assert_eq!(
            output,
            "id: 42\nevent:  update\nretry: 1234\ndata: payload\n\n",
        );
    }

    #[test]
    fn encodes_comments() {
        for (comment, expected) in [
            ("", ": \n\n"),
            ("keep-alive", ": keep-alive\n\n"),
            (
                "\nfirst\r\nsecond\r\n\rthird\n",
                ": \n: first\n: second\n: \n: third\n: \n\n",
            ),
        ] {
            let mut output = BytesMut::new();

            Encoder::default()
                .encode(Event::Comment(comment.into()), &mut output)
                .unwrap();

            assert_eq!(output, expected, "input: {comment:?}");
        }
    }

    #[test]
    fn encodes_retry_delays_in_milliseconds() {
        for (retry, expected) in [
            (Duration::ZERO, "retry: 0\n\n"),
            (Duration::from_secs(10), "retry: 10000\n\n"),
            (Duration::from_micros(1_999), "retry: 1\n\n"),
            (Duration::MAX, "retry: 18446744073709551615999\n\n"),
        ] {
            let mut output = BytesMut::new();

            Encoder::default()
                .encode(Event::Retry(retry), &mut output)
                .unwrap();

            assert_eq!(output, expected);
        }
    }

    #[test]
    fn rejects_invalid_message_fields_without_changing_output() {
        for (id, event) in [
            ("bad\nid", "update"),
            ("bad\rid", "update"),
            ("bad\r\nid", "update"),
            ("bad\0id", "update"),
            ("42", "bad\nevent"),
            ("42", "bad\revent"),
            ("42", "bad\r\nevent"),
            ("é\0id", "bad\nevent"),
            ("é", "世界\revent"),
        ] {
            let message = Message {
                data: "payload".into(),
                event: Some(event.into()),
                retry: Some(Duration::from_secs(1)),
                id: Some(id.into()),
            };
            let mut output = BytesMut::from("data: previous\n\n");

            let result = Encoder::default().encode(Event::Message(message), &mut output);

            assert!(
                matches!(result, Err(Error::InvalidFieldValue { .. })),
                "got: {result:?}",
            );
            assert_eq!(output, "data: previous\n\n");
        }
    }

    #[test]
    fn encodes_empty_message_fields() {
        let message = Message {
            data: "".into(),
            event: Some("".into()),
            retry: Some(Duration::ZERO),
            id: Some("".into()),
        };
        let mut output = BytesMut::new();

        Encoder::default()
            .encode(Event::Message(message), &mut output)
            .unwrap();

        assert_eq!(output, "id: \nevent: \nretry: 0\ndata: \n\n");
    }

    #[test]
    fn appends_utf8_and_normalized_lines_to_existing_output() {
        for event in [
            Event::Message(Message::data("é\r\n世界\r🦀\n")),
            Event::Comment("é\r\n世界\r🦀\n".into()),
            Event::Retry(Duration::MAX),
            Event::Message(Message {
                id: Some("".into()),
                event: Some("".into()),
                retry: Some(Duration::MAX),
                ..Message::data("")
            }),
        ] {
            let expected = event.clone().into_bytestring().unwrap();
            let mut output = BytesMut::from("data: previous\n\n");

            Encoder::default().encode(event, &mut output).unwrap();

            assert_eq!(&output[..16], b"data: previous\n\n");
            assert_eq!(&output[16..], expected.as_bytes());
        }
    }

    #[tokio::test]
    async fn writes_events_with_framed_write() {
        let mut framed = FramedWrite::new(Vec::new(), Encoder::default());

        framed
            .feed(Event::Retry(Duration::from_secs(1)))
            .await
            .unwrap();
        framed
            .feed(Event::Comment("keep-alive".into()))
            .await
            .unwrap();
        framed
            .feed(Event::Message(Message {
                data: "first\nsecond\n".into(),
                event: Some("update".into()),
                retry: None,
                id: Some("42".into()),
            }))
            .await
            .unwrap();
        framed.flush().await.unwrap();

        assert_eq!(
            framed.into_inner(),
            b"retry: 1000\n\n: keep-alive\n\nid: 42\nevent: update\ndata: first\ndata: second\ndata: \n\n",
        );
    }

    #[test]
    fn encoded_events_can_be_decoded() {
        let events = [
            Event::Retry(Duration::from_secs(1)),
            Event::Comment("keep-alive".into()),
            Event::Message(Message {
                data: "first\nsecond\n".into(),
                event: Some("update".into()),
                retry: Some(Duration::from_millis(42)),
                id: Some("123".into()),
            }),
        ];
        let mut output = BytesMut::new();
        let mut encoder = Encoder::default();
        let mut decoder = crate::Decoder::default();

        for event in &events {
            encoder.encode(event.clone(), &mut output).unwrap();
        }

        for event in events {
            assert_eq!(decoder.decode(&mut output).unwrap(), Some(event));
        }

        assert!(output.is_empty());
    }
}
