//! Blocking SSE iterator for ureq v3.
//!
//! The [extension trait](UreqExt) provides a fluent API to chain [`.sse_iter()`](UreqExt::sse_iter)
//! on a response.
//!
//! Enable TLS features on your `ureq` dependency if you need HTTPS.

use std::io::{self, Read as _};

use bytes::BytesMut;
use tokio_util::codec::Decoder as _;
use ureq_3::{Body, BodyReader, http::Response};

use crate::{DecodeError, Decoder, Event};

mod sealed {
    use super::*;

    pub trait Sealed {}
    impl Sealed for Response<Body> {}
}

/// SSE extension methods for ureq v3.
pub trait UreqExt: sealed::Sealed {
    /// Returns a blocking iterator of server-sent events.
    ///
    /// Each call to `next()` can block until an event, an error, or the end of the response body.
    /// The iterator ends after an I/O or decoding error. The iterator owns the response body and
    /// can move to another thread.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use russe::ureq_3::UreqExt as _;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let events = ureq_3::get("http://httpbin.org/sse").call()?.sse_iter();
    ///
    /// for event in events {
    ///     println!("{:?}", event?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn sse_iter(self) -> impl Iterator<Item = Result<Event, DecodeError>> + Send + 'static;
}

impl UreqExt for Response<Body> {
    fn sse_iter(self) -> impl Iterator<Item = Result<Event, DecodeError>> + Send + 'static {
        SseIter {
            reader: self.into_body().into_reader(),
            decoder: Decoder::default(),
            buf: BytesMut::new(),
            eof: false,
            done: false,
        }
    }
}

struct SseIter {
    reader: BodyReader<'static>,
    decoder: Decoder,
    buf: BytesMut,
    eof: bool,
    done: bool,
}

impl Iterator for SseIter {
    type Item = Result<Event, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        loop {
            let event = if self.eof {
                self.decoder.decode_eof(&mut self.buf)
            } else {
                self.decoder.decode(&mut self.buf)
            };

            match event {
                Ok(Some(event)) => return Some(Ok(event)),

                Ok(None) if self.eof => {
                    self.done = true;

                    return None;
                }

                Err(err) => {
                    self.done = true;

                    return Some(Err(err));
                }

                Ok(None) => {}
            }

            let mut chunk = [0; 8_192];

            match self.reader.read(&mut chunk) {
                Ok(0) => self.eof = true,

                Ok(len) => self.buf.extend_from_slice(&chunk[..len]),

                Err(err) if err.kind() == io::ErrorKind::Interrupted => {}

                Err(err) => {
                    self.done = true;

                    return Some(Err(err.into()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::{self, Read},
        time::Duration,
    };

    use indoc::indoc;
    use ureq_3::{Agent, Body, http::Response};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::UreqExt as _;
    use crate::{DecodeError, Event, Message};

    struct ScriptedReader(VecDeque<io::Result<&'static [u8]>>);

    impl Read for ScriptedReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.0.pop_front() {
                Some(Ok(chunk)) => {
                    assert!(chunk.len() <= buf.len());

                    buf[..chunk.len()].copy_from_slice(chunk);

                    Ok(chunk.len())
                }

                Some(Err(err)) => Err(err),

                None => Ok(0),
            }
        }
    }

    #[test]
    fn response_iterator_decodes_sse_events() {
        let body = Body::builder().data(indoc! {"
            id: 42
            event: update
            data: hello

            : heartbeat

            retry: 1000

        "});

        let events = Response::new(body)
            .sse_iter()
            .collect::<Result<Vec<_>, _>>()
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
                Event::Retry(Duration::from_secs(1)),
            ],
        );
    }

    #[test]
    fn decodes_fragmented_utf8_and_crlf_delimiters() {
        let chunks: &[&[u8]] = &[
            b"unknown: ignored\n\n: heart",
            b"beat\r\ndata: caf\xc3",
            b"\xa9\r\n\r",
            b"\n",
        ];

        let body = Body::builder().reader(ScriptedReader(
            chunks.iter().map(|&chunk| Ok(chunk)).collect(),
        ));

        let events = Response::new(body)
            .sse_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            events,
            vec![
                Event::Comment("heartbeat".into()),
                Event::Message(Message {
                    data: "café".into(),
                    event: None,
                    retry: None,
                    id: None,
                }),
            ],
        );
    }

    #[test]
    fn yields_buffered_events_before_reading_again() {
        let body = Body::builder().reader(ScriptedReader(VecDeque::from([
            Ok(indoc! {b"
                : heartbeat
                data: hello

            "}
            .as_slice()),
            Err(io::Error::other("body read failed")),
        ])));

        let mut events = Response::new(body).sse_iter();

        assert_eq!(
            events.next().unwrap().unwrap(),
            Event::Comment("heartbeat".into()),
        );

        assert_eq!(
            events.next().unwrap().unwrap(),
            Event::Message(Message {
                data: "hello".into(),
                event: None,
                retry: None,
                id: None,
            }),
        );

        let err = events.next().unwrap().unwrap_err();

        assert!(
            matches!(err, DecodeError::Io { .. }),
            "expected I/O error, got: {err:?}"
        );

        assert!(events.next().is_none());
        assert!(events.next().is_none());
    }

    #[test]
    fn retries_interrupted_reads() {
        let body = Body::builder().reader(ScriptedReader(VecDeque::from([
            Err(io::Error::from(io::ErrorKind::Interrupted)),
            Ok(b"data: hello\n\n".as_slice()),
        ])));

        let events = Response::new(body)
            .sse_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

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

    #[test]
    fn returns_invalid_utf8_error_once() {
        let body = Body::builder().data(
            indoc! {b"
            data: \xff

            data: ignored

        "}
            .as_slice(),
        );

        let mut events = Response::new(body).sse_iter();
        let err = events.next().unwrap().unwrap_err();

        assert!(
            matches!(err, DecodeError::InvalidUtf8 { .. }),
            "expected invalid UTF-8 error, got: {err:?}"
        );

        assert!(events.next().is_none());
        assert!(events.next().is_none());
    }

    #[test]
    fn returns_incomplete_frame_error_once() {
        let body = Body::builder().data("data: incomplete\n");

        let mut events = Response::new(body).sse_iter();
        let err = events.next().unwrap().unwrap_err();

        assert!(
            matches!(err, DecodeError::Io { .. }),
            "expected I/O error, got: {err:?}"
        );

        assert!(events.next().is_none());
        assert!(events.next().is_none());
    }

    #[test]
    fn empty_body_ends_iteration() {
        let body = Body::builder().data("");

        let mut events = Response::new(body).sse_iter();

        assert!(events.next().is_none());
        assert!(events.next().is_none());
    }

    #[tokio::test]
    async fn decodes_event_from_http_response() {
        let server = MockServer::start().await;
        let body = indoc! {"
            data: hello

        "};

        Mock::given(method("GET"))
            .and(path("/events"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, crate::MEDIA_TYPE_STR))
            .expect(1)
            .mount(&server)
            .await;

        let url = format!("{}/events", server.uri());

        let events = tokio::task::spawn_blocking(move || {
            let agent = Agent::config_builder()
                .proxy(None)
                .timeout_global(Some(Duration::from_secs(5)))
                .build()
                .new_agent();

            agent
                .get(&url)
                .call()
                .unwrap()
                .sse_iter()
                .collect::<Result<Vec<_>, _>>()
        })
        .await
        .unwrap()
        .unwrap();

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
}
