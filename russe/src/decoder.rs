use std::{str, time::Duration};

use aho_corasick::AhoCorasick;
use bytes::{Buf as _, Bytes, BytesMut};
use bytestring::ByteString;

use crate::{Error, NEWLINE, SSE_DELIMITER, event::Event, message::Message};

/// SSE decoder.
///
/// Comment lines within a frame are combined into one [`Event::Comment`] and emitted
/// before its message or retry event, regardless of their position. Decoding and
/// re-encoding can therefore produce different bytes.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Decoder {
    event_finder: AhoCorasick,
    pending_event: Option<Event>,
    skip_lf: bool,
}

impl Default for Decoder {
    fn default() -> Self {
        Self {
            event_finder: AhoCorasick::new([SSE_DELIMITER, b"\n\r", b"\r\r"]).unwrap(),
            pending_event: None,
            skip_lf: false,
        }
    }
}

impl tokio_util::codec::Decoder for Decoder {
    type Item = Event;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(event) = self.pending_event.take() {
            return Ok(Some(event));
        }

        // Skip empty or unknown-only frames without waiting for more input.
        loop {
            if self.skip_lf && !src.is_empty() {
                self.skip_lf = false;

                if src[0] == NEWLINE {
                    src.advance(1);
                }
            }

            // Find a blank line, or wait for more data.
            let Some(delimiter) = self.event_finder.find(&*src) else {
                tracing::trace!(buffered_bytes = src.len(), "Not enough data in buffer");
                return Ok(None);
            };

            // full message received; remove from src buffer
            let ends_with_cr = src[delimiter.end() - 1] == b'\r';
            let buf = src.split_to(delimiter.start());

            // remove the delimiter from the buffer too
            drop(src.split_to(delimiter.len()));

            self.skip_lf = ends_with_cr;

            if self.skip_lf && src.first() == Some(&NEWLINE) {
                src.advance(1);
                self.skip_lf = false;
            }

            // Frame boundaries are already consumed. Skip empty segments from CRLF.
            let lines = buf
                .as_ref()
                .split(|&byte| matches!(byte, b'\r' | b'\n'))
                .filter(|line| !line.is_empty());

            let mut message = Message {
                retry: None,
                event: None,
                data: ByteString::new(),
                id: None,
            };

            // TODO: if optimistic buffering is desired then remove this
            let mut data_buf = BytesMut::with_capacity(64);
            let mut comment_buf = BytesMut::new();

            // SSE requires unknown fields to be ignored. Track recognized message
            // fields so empty or unknown-only frames do not emit a message.
            // https://html.spec.whatwg.org/multipage/server-sent-events.html#event-stream-interpretation
            let mut message_event = false;

            for line in lines {
                let mut line = Bytes::copy_from_slice(line);

                let input = if let Some(colon) = memchr::memchr(b':', &line) {
                    let mut input = line.split_off(colon + 1);
                    line.truncate(colon);

                    if input.first() == Some(&b' ') {
                        input.advance(1);
                    }

                    input
                } else {
                    // A field without a colon has an empty value.
                    Bytes::new()
                };

                match line.as_ref() {
                    b"data" => {
                        data_buf.extend_from_slice(&input);
                        data_buf.extend_from_slice(&[NEWLINE]);

                        message_event = true;
                    }

                    b"id" => {
                        let id = ByteString::try_from(input)?;

                        message.id = Some(id);
                        message_event = true;
                    }

                    b"event" => {
                        let event = ByteString::try_from(input)?;

                        message.event = Some(event);
                        message_event = true;
                    }

                    b"retry" => {
                        // SSE accepts only nonempty ASCII digit sequences for retry.
                        if input.is_empty() || !input.iter().all(u8::is_ascii_digit) {
                            continue;
                        }

                        let input = str::from_utf8(&input)?;

                        // Duration can hold more than u64::MAX milliseconds.
                        // Reject only values that exceed its seconds range.
                        if let Ok(millis) = input.parse::<u128>()
                            && let Ok(seconds) = u64::try_from(millis / 1_000)
                        {
                            let nanos = (millis % 1_000) as u32 * 1_000_000;

                            message.retry = Some(Duration::new(seconds, nanos));
                        }
                    }

                    // comment
                    b"" => {
                        comment_buf.extend_from_slice(&input);
                        comment_buf.extend_from_slice(&[NEWLINE]);
                    }

                    _ => {}
                }
            }

            if !data_buf.is_empty() {
                data_buf.truncate(data_buf.len() - 1);

                let data = ByteString::try_from(data_buf)?;

                message.data = data;
            }

            if !comment_buf.is_empty() {
                comment_buf.truncate(comment_buf.len() - 1);

                let comment = ByteString::try_from(comment_buf)?;

                self.pending_event = if message_event {
                    Some(Event::Message(message))
                } else {
                    message.retry.map(Event::Retry)
                };

                return Ok(Some(Event::Comment(comment)));
            }

            match message.retry {
                Some(retry) if !message_event => return Ok(Some(Event::Retry(retry))),
                _ => {}
            }

            if message_event {
                return Ok(Some(Event::Message(message)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error as _, io, pin::pin};

    use bytes::Bytes;
    use futures_test::stream::StreamTestExt as _;
    use futures_util::{StreamExt as _, stream};
    use indoc::{formatdoc, indoc};
    use tokio_util::{
        codec::{Decoder as _, FramedRead},
        io::StreamReader,
    };

    use super::*;
    use crate::assert_none;

    #[test]
    fn preserves_leading_empty_data_lines() {
        for (input, expected) in [
            ("data: \ndata: \n\n", "\n"),
            ("data: \ndata: hello\n\n", "\nhello"),
        ] {
            let mut input = BytesMut::from(input);

            let event = Decoder::default().decode(&mut input).unwrap();

            assert_eq!(event, Some(Event::Message(Message::data(expected))));
        }
    }

    #[test]
    fn null_field_does_not_panic() {
        let mut input = BytesMut::from("\0\n\n");

        let _ = Decoder::default().decode(&mut input);
    }

    #[test]
    fn preserves_multiline_comments() {
        let mut input = BytesMut::from(indoc! {"
            : first
            : second

        "});
        let mut decoder = Decoder::default();
        let mut comments = Vec::new();

        while let Some(event) = decoder.decode(&mut input).unwrap() {
            let Event::Comment(comment) = event else {
                panic!("expected comment, got: {event:?}");
            };

            comments.push(comment.to_string());
        }

        assert_eq!(comments.join("\n"), "first\nsecond");
    }

    #[test]
    fn comments_do_not_discard_message_data() {
        for (input, comment) in [
            (
                indoc! {"
                    : heartbeat
                    data: hello

                "},
                "heartbeat",
            ),
            (
                indoc! {"
                    data: hello
                    : heartbeat

                "},
                "heartbeat",
            ),
            (
                indoc! {"
                    : first
                    data: hello
                    : second

                "},
                "first\nsecond",
            ),
            (
                indoc! {"
                    data: hello
                    : first
                    : second

                "},
                "first\nsecond",
            ),
        ] {
            let mut input = BytesMut::from(input);
            input.extend_from_slice(indoc! {b"
                data: next

            "});

            let mut decoder = Decoder::default();

            let events =
                std::iter::from_fn(|| decoder.decode(&mut input).unwrap()).collect::<Vec<_>>();

            assert_eq!(
                events,
                [
                    Event::Comment(comment.into()),
                    Event::Message(Message::data("hello")),
                    Event::Message(Message::data("next")),
                ],
            );
        }
    }

    #[test]
    fn comments_do_not_discard_retry_values() {
        for (input, comment) in [
            (
                indoc! {"
                    : heartbeat
                    retry: 42

                "},
                "heartbeat",
            ),
            (
                indoc! {"
                    retry: 42
                    : heartbeat

                "},
                "heartbeat",
            ),
            (
                indoc! {"
                    : first
                    retry: 42
                    : second

                "},
                "first\nsecond",
            ),
        ] {
            let mut input = BytesMut::from(input);
            input.extend_from_slice(indoc! {b"
                data: next

            "});

            let mut decoder = Decoder::default();
            let events =
                std::iter::from_fn(|| decoder.decode(&mut input).unwrap()).collect::<Vec<_>>();

            assert_eq!(
                events,
                [
                    Event::Comment(comment.into()),
                    Event::Retry(Duration::from_millis(42)),
                    Event::Message(Message::data("next"))
                ]
            );
        }
    }

    #[test]
    fn decodes_crlf_line_endings() {
        let mut input = BytesMut::from(concat! {
            "data: hello\r\n",
            "\r\n",
        });

        let event = Decoder::default().decode(&mut input).unwrap();

        assert_eq!(event, Some(Event::Message(Message::data("hello"))));
    }

    #[test]
    fn decodes_cr_line_endings() {
        for (input, expected) in [
            (
                concat! {
                    "data: hello\r",
                    "\r",
                },
                "hello",
            ),
            (
                concat! {
                    "data: first\r",
                    "data: second\r",
                    "\r",
                },
                "first\nsecond",
            ),
            (
                concat! {
                    "data: first\r\n",
                    "data: second\r",
                    "\r",
                },
                "first\nsecond",
            ),
            (
                concat! {
                    "data: hello\n",
                    "\r",
                },
                "hello",
            ),
            (
                concat! {
                    "data: hello\r\n",
                    "\r",
                },
                "hello",
            ),
        ] {
            let mut input = BytesMut::from(input);
            let mut decoder = Decoder::default();

            let event = decoder.decode(&mut input).unwrap();

            assert_eq!(event, Some(Event::Message(Message::data(expected))));

            input.extend_from_slice(
                concat! {
                    "\n",
                    "data: next\r",
                    "\r",
                }
                .as_bytes(),
            );

            assert_eq!(
                decoder.decode(&mut input).unwrap(),
                Some(Event::Message(Message::data("next"))),
            );
        }
    }

    #[test]
    fn ignores_unknown_fields() {
        for input in [
            indoc! {"
                extension: ignored
                data: hello

            "},
            indoc! {"
                xdata: ignored
                data: hello

            "},
            indoc! {"
                extension: ignored

                data: hello

            "},
            indoc! {"


                data: hello

            "},
        ] {
            let mut input = BytesMut::from(input);

            let event = Decoder::default().decode(&mut input).unwrap();

            assert_eq!(event, Some(Event::Message(Message::data("hello"))));
        }

        let mut input = BytesMut::from(indoc! {"
            extension: ignored

        "});

        assert_none!(Decoder::default().decode(&mut input).unwrap());
    }

    #[test]
    fn decodes_data_without_a_colon() {
        for (input, expected) in [
            (
                indoc! {"
                    data

                "},
                "",
            ),
            (
                indoc! {"
                    data
                    data: hello

                "},
                "\nhello",
            ),
            (
                indoc! {"
                    data: hello
                    data

                "},
                "hello\n",
            ),
        ] {
            let mut input = BytesMut::from(input);

            let event = Decoder::default().decode(&mut input).unwrap();

            assert_eq!(event, Some(Event::Message(Message::data(expected))));
        }
    }

    #[quickcheck_macros::quickcheck]
    fn arbitrary_retry_values_do_not_panic(mut value: Vec<u8>) {
        for byte in &mut value {
            if *byte == b'\r' || *byte == b'\n' {
                *byte = b' ';
            }
        }

        let mut input = BytesMut::from(b"retry: ".as_slice());
        input.extend_from_slice(&value);
        input.extend_from_slice(indoc! {b"

            data: hello

        "});

        // Invalid UTF-8 may return an error, but a retry value must not cause a panic.
        let _ = Decoder::default().decode(&mut input);
    }

    #[test]
    fn ignores_invalid_retry_values() {
        for retry in ["invalid", "", "-1", "+1", "1.5", " 1", "١"] {
            let frame = formatdoc! {"
                retry: {retry}
                data: hello

            "};
            let mut input = BytesMut::from(frame.as_str());

            let event = Decoder::default().decode(&mut input).unwrap();

            assert_eq!(event, Some(Event::Message(Message::data("hello"))));
        }

        let mut input = BytesMut::from(indoc! {"
            retry: 42
            retry: invalid
            data: hello

        "});

        let event = Decoder::default().decode(&mut input).unwrap();

        assert_eq!(
            event,
            Some(Event::Message(Message {
                retry: Some(Duration::from_millis(42)),
                ..Message::data("hello")
            })),
        );
    }

    #[test]
    fn decodes_retry_larger_than_u64_milliseconds() {
        for (millis, expected) in [
            (
                "18446744073709552000",
                Some(Duration::from_secs(18_446_744_073_709_552)),
            ),
            (
                "18446744073709552001",
                Some(Duration::new(18_446_744_073_709_552, 1_000_000)),
            ),
            (
                "18446744073709551615999",
                Some(Duration::new(u64::MAX, 999_000_000)),
            ),
            ("18446744073709551616000", None),
            ("340282366920938463463374607431768211456", None),
        ] {
            let frame = formatdoc! {"
                retry: {millis}
                data: hello

            "};
            let mut input = BytesMut::from(frame.as_str());

            let event = Decoder::default().decode(&mut input).unwrap();

            assert_eq!(
                event,
                Some(Event::Message(Message {
                    retry: expected,
                    ..Message::data("hello")
                }))
            );
        }
    }

    #[tokio::test]
    async fn reads_sse_frames() {
        let input = indoc! {"
            retry: 444

            : begin by specifying retry duration

            data: msg1 simple

            data: msg2
            data: with more on a newline

            data:msg3 without optional leading space

            data: msg4 with an ID
            id: 42

            retry: 999
            data: msg5 specifies new retry
            id: 43a

            event: msg
            data: msg6 is named

        "};

        assert!(input.as_bytes().ends_with(SSE_DELIMITER));

        let body_stream = stream::iter(input.as_bytes().chunks(7))
            .map(|line| Ok::<_, io::Error>(Bytes::from(line)))
            .interleave_pending();
        let body_reader = StreamReader::new(body_stream);

        let event_stream = FramedRead::new(body_reader, Decoder::default());
        let mut event_stream = pin!(event_stream);

        let ev = event_stream.next().await.unwrap().unwrap();
        assert_eq!(Event::Retry(Duration::from_millis(444)), ev);

        let ev = event_stream.next().await.unwrap().unwrap();
        assert_eq!(
            Event::Comment("begin by specifying retry duration".into()),
            ev,
        );

        let ev = event_stream.next().await.unwrap().unwrap();
        assert_eq!(Event::Message(Message::data("msg1 simple")), ev);

        let ev = event_stream.next().await.unwrap().unwrap();
        assert_eq!(
            Event::Message(Message::data("msg2\nwith more on a newline")),
            ev,
        );

        let ev = event_stream.next().await.unwrap().unwrap();
        assert_eq!(
            Event::Message(Message::data("msg3 without optional leading space")),
            ev,
        );

        let ev = event_stream.next().await.unwrap().unwrap();
        assert_eq!(
            Event::Message(Message {
                data: "msg4 with an ID".into(),
                id: Some("42".into()),
                ..Default::default()
            }),
            ev,
        );

        let ev = event_stream.next().await.unwrap().unwrap();
        assert_eq!(
            Event::Message(Message {
                data: "msg5 specifies new retry".into(),
                id: Some("43a".into()),
                retry: Some(Duration::from_millis(999)),
                event: None,
            }),
            ev,
        );

        let ev = event_stream.next().await.unwrap().unwrap();
        assert_eq!(
            Event::Message(Message {
                data: "msg6 is named".into(),
                event: Some("msg".into()),
                ..Default::default()
            }),
            ev,
        );

        // no more events in the stream
        assert_none!(event_stream.next().await);
    }

    #[tokio::test]
    async fn errors_on_invalid_utf8() {
        let input = b"data: invalid\xC3\x28msg\n\n".as_slice();

        assert!(input.ends_with(SSE_DELIMITER));

        let body_stream =
            stream::once(async { input }).map(|line| Ok::<_, io::Error>(Bytes::from(line)));
        let body_reader = StreamReader::new(body_stream);

        let event_stream = FramedRead::new(body_reader, Decoder::default());
        let mut event_stream = pin!(event_stream);

        let err = event_stream.next().await.unwrap().unwrap_err();

        let Error::InvalidUtf8 { source } = &err else {
            panic!("expected invalid UTF-8 error, got: {err:?}");
        };

        assert_eq!(source.valid_up_to(), 7);
        assert_eq!(source.error_len(), Some(1));
        assert_eq!(
            err.source().unwrap().downcast_ref::<str::Utf8Error>(),
            Some(source),
        );
        assert_eq!(err.to_string(), "Stream contained invalid UTF-8");

        // no more events in the stream
        assert_none!(event_stream.next().await);
    }
}
