use std::{
    io, mem,
    pin::Pin,
    task::{Poll, ready},
    time::Duration,
};

use bytes::Bytes;
use bytestring::ByteString;
use futures_util::{Stream, stream};
use memchr::memchr2;

use crate::{
    CR, Error, Event, LF, Message,
    fields::{data_event, field_value, parse_retry, strip_space},
};

/// Parse a response body one byte chunk at a time.
pub(crate) fn decode_body<S>(mut body: S) -> impl Stream<Item = Result<Event, Error>>
where
    S: Stream<Item = io::Result<Bytes>> + Unpin,
{
    let mut decoder = BodyDecoder::default();
    let mut chunk = Bytes::new();
    let mut cursor = 0;
    let mut done = false;

    stream::poll_fn(move |cx| {
        if done {
            return Poll::Ready(None);
        }

        loop {
            match decoder.decode(&chunk, &mut cursor) {
                Ok(Some(event)) => return Poll::Ready(Some(Ok(event))),
                Ok(None) => {}
                Err(err) => {
                    done = true;
                    return Poll::Ready(Some(Err(err)));
                }
            }

            chunk.clear();

            match ready!(Pin::new(&mut body).poll_next(cx)) {
                Some(Ok(bytes)) => {
                    chunk = bytes;
                    cursor = 0;
                }
                Some(Err(err)) => {
                    done = true;
                    return Poll::Ready(Some(Err(err.into())));
                }
                None => {
                    done = true;
                    let error = decoder
                        .has_incomplete_frame()
                        .then(|| Err(io::Error::other("bytes remaining on stream").into()));
                    return Poll::Ready(error);
                }
            }
        }
    })
}

#[derive(Clone, Copy, Default)]
enum Line {
    #[default]
    Start,
    Data,
    // The chunk ended just after "data:".
    DataSpace,
    // A field line with a split prefix or a non-data value.
    Partial,
}

#[derive(Default)]
struct BodyDecoder {
    data: Vec<u8>,
    next_capacity: usize,
    line: Line,
    has_data: bool,
    after_line: bool,
    skip_lf: bool,
    partial: Vec<u8>,
    comments: Option<Vec<u8>>,
    id: Option<ByteString>,
    event: Option<ByteString>,
    retry: Option<Duration>,
    field_error: Option<std::str::Utf8Error>,
    pending: Option<Event>,
}

impl BodyDecoder {
    // Bound the capacity reserved for the event after a large payload.
    const MAX_CAPACITY_HINT: usize = 128 * 1024;

    fn has_incomplete_frame(&self) -> bool {
        self.after_line || !matches!(self.line, Line::Start)
    }

    fn decode(&mut self, chunk: &Bytes, cursor: &mut usize) -> Result<Option<Event>, Error> {
        if let Some(event) = self.pending.take() {
            return Ok(Some(event));
        }

        while let Some(&first) = chunk.get(*cursor) {
            if mem::take(&mut self.skip_lf) && first == LF {
                *cursor += 1;
                continue;
            }

            if matches!(self.line, Line::Start) && matches!(first, CR | LF) {
                *cursor += 1;
                self.skip_lf = first == CR;

                // A second line break ends the frame.
                if self.after_line {
                    self.after_line = false;
                    if let Some(event) = self.finish_frame()? {
                        return Ok(Some(event));
                    }
                } else {
                    self.after_line = true;
                }
                continue;
            }

            let rest = &chunk[*cursor..];
            let end = memchr2(CR, LF, rest);
            let part = &rest[..end.unwrap_or(rest.len())];

            match self.line {
                Line::Data => self.data.extend_from_slice(part),
                Line::DataSpace => {
                    self.data.extend_from_slice(strip_space(part));
                    self.line = Line::Data;
                }
                Line::Partial => {
                    self.partial.extend_from_slice(part);
                    if end.is_some() {
                        let mut line = mem::take(&mut self.partial);
                        self.field(&line);
                        line.clear();
                        self.partial = line;
                    }
                }
                Line::Start => {
                    if let Some(value) = part.strip_prefix(b"data:") {
                        let value = strip_space(value);
                        if !self.after_line
                            && let Some(end) = end
                            && rest[end..].starts_with(b"\n\n")
                        {
                            let value_start = *cursor + part.len() - value.len();
                            *cursor += end + 2;
                            let data = if value.len() >= chunk.len() / 2 {
                                ByteString::try_from(
                                    chunk.slice(value_start..value_start + value.len()),
                                )
                            } else if value.len() >= 1024 {
                                // Standard UTF-8 validation is faster for large slices.
                                std::str::from_utf8(value).map(ByteString::from)
                            } else {
                                ByteString::try_from(value)
                            };
                            self.next_capacity = value.len().min(Self::MAX_CAPACITY_HINT);
                            return Ok(Some(data_event(data?)));
                        }

                        self.start_data();
                        self.data.extend_from_slice(value);
                        self.line = if end.is_none() && part.len() == b"data:".len() {
                            Line::DataSpace
                        } else {
                            Line::Data
                        };
                    } else if end.is_some() {
                        self.field(part);
                    } else {
                        self.partial.extend_from_slice(part);
                        self.line = Line::Partial;
                    }
                }
            }

            match end {
                Some(index) => {
                    self.skip_lf = rest[index] == CR;
                    *cursor += index + 1;
                    self.after_line = true;
                    self.line = Line::Start;
                }
                None => {
                    *cursor = chunk.len();
                    self.after_line = false;
                }
            }
        }

        Ok(None)
    }

    fn start_data(&mut self) {
        if self.has_data {
            self.data.push(LF);
        } else {
            self.has_data = true;
            if self.data.capacity() == 0 {
                self.data.reserve_exact(self.next_capacity);
            }
        }
    }

    fn field(&mut self, line: &[u8]) {
        if self.field_error.is_some() {
            return;
        }
        match line {
            [b'd', ..] => {
                if let Some(value) = field_value(line, b"data") {
                    self.start_data();
                    self.data.extend_from_slice(value);
                }
            }
            [b':', value @ ..] => {
                let value = strip_space(value);
                if let Some(comments) = &mut self.comments {
                    comments.push(LF);
                    comments.extend_from_slice(value);
                } else {
                    self.comments = Some(value.to_vec());
                }
            }
            [b'i', ..] => {
                if let Some(value) = field_value(line, b"id") {
                    match ByteString::try_from(value) {
                        Ok(value) => self.id = Some(value),
                        Err(err) => self.field_error = Some(err),
                    }
                }
            }
            [b'e', ..] => {
                if let Some(value) = field_value(line, b"event") {
                    match ByteString::try_from(value) {
                        Ok(value) => self.event = Some(value),
                        Err(err) => self.field_error = Some(err),
                    }
                }
            }
            [b'r', ..] => {
                if let Some(delay) = field_value(line, b"retry").and_then(parse_retry) {
                    self.retry = Some(delay);
                }
            }
            _ => {}
        }
    }

    fn finish_frame(&mut self) -> Result<Option<Event>, Error> {
        if let Some(err) = self.field_error.take() {
            return Err(err.into());
        }

        let data = if mem::take(&mut self.has_data) {
            self.next_capacity = self.data.len().min(Self::MAX_CAPACITY_HINT);
            let data =
                String::from_utf8(mem::take(&mut self.data)).map_err(|err| err.utf8_error())?;
            Some(ByteString::from(data))
        } else {
            None
        };
        let comment = self.comments.take().map(ByteString::try_from).transpose()?;
        let event = self.event.take();
        let id = self.id.take();
        let retry = self.retry.take();

        let event = if data.is_some() || event.is_some() || id.is_some() {
            Some(Event::Message(Message {
                data: data.unwrap_or_default(),
                event,
                retry,
                id,
            }))
        } else {
            retry.map(Event::Retry)
        };

        if let Some(comment) = comment {
            self.pending = event;
            Ok(Some(Event::Comment(comment)))
        } else {
            Ok(event)
        }
    }
}

#[cfg(test)]
mod tests {
    use futures_test::stream::StreamTestExt as _;
    use futures_util::{FutureExt as _, StreamExt as _};
    use tokio_util::{codec::FramedRead, io::StreamReader};

    use super::*;
    use crate::Decoder;

    #[test]
    fn complete_chunk_yields_an_event_before_polling_the_body_again() {
        let input = Bytes::from_static(b"data: hello world\n\n");
        let payload = input[b"data: ".len()..].as_ptr();
        let mut chunk = Some(input);
        let body = stream::poll_fn(move |_| match chunk.take() {
            Some(bytes) => Poll::Ready(Some(Ok(bytes))),
            None => Poll::Pending,
        });
        let mut events = decode_body(body);

        let event = events.next().now_or_never().unwrap().unwrap().unwrap();
        assert_eq!(event, data_event("hello world".into()));
        assert!(matches!(event, Event::Message(message) if message.data.as_ptr() == payload));
    }

    #[tokio::test]
    async fn matches_the_codec_across_chunk_boundaries() {
        let fixtures = [
            b"retry: 42\r\r\nid: 7\r\ndata: caf\xc3\xa9\ndata\r: first\r\n: second\nevent: named\n\r\ndata: next\n\n".as_slice(),
            b"data: ok\nid: \xff\n\n",
            b"data: valid\n: \xff\n\n",
            b"data: \xff\n: bad\n\n",
            b"data:          hello\n\n",
            b"data: unfinished\n",
            b"\n",
            b"data: hello\r\n\r",
        ];

        for input in fixtures {
            for chunk_size in 1..=input.len() {
                let body = || {
                    stream::iter(input.chunks(chunk_size))
                        .map(|data| Ok::<_, io::Error>(Bytes::from_static(data)))
                        .interleave_pending()
                };

                let expected = FramedRead::new(StreamReader::new(body()), Decoder::default())
                    .map(|event| format!("{event:?}"))
                    .collect::<Vec<_>>()
                    .await;

                let actual = decode_body(body())
                    .map(|event| format!("{event:?}"))
                    .collect::<Vec<_>>()
                    .await;

                assert_eq!(actual, expected, "{input:?} with chunk size {chunk_size}");
            }
        }
    }
}
