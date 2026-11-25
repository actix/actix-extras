use std::{mem, ops::Range, str, time::Duration};

use bytes::{Buf as _, BytesMut};
use bytestring::ByteString;
use memchr::{memchr2, memchr2_iter};

use crate::{
    CR, Error, LF,
    event::Event,
    fields::{data_event, field_value, parse_id, parse_retry, strip_space},
    message::Message,
};

/// SSE decoder.
///
/// Comment lines within a frame are combined into one [`Event::Comment`] and emitted
/// before its message or retry event, regardless of their position. Decoding and
/// re-encoding can therefore produce different bytes.
///
/// If you call the codec directly, keep unread bytes in the buffer and append new bytes to them.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Decoder {
    pending_event: Option<Event>,
    scan_from: usize,
    last_line_start: usize,
    frame_kind: FrameKind,
    data_lines: DataLines,
    skip_lf: bool,
}

#[derive(Debug, Clone, Copy, Default)]
enum FrameKind {
    #[default]
    SingleLine,
    DataLines,
    Other,
}

#[derive(Debug, Clone, Default)]
struct DataLines {
    values: Vec<Range<usize>>,
    next_line: usize,
    len: usize,
}

impl DataLines {
    // Bound the metadata retained for a frame with many short lines.
    const MAX_LINES: usize = 1024;

    /// Records the byte range of the next `data` line ending at `end` in `src`.
    ///
    /// Returns true if the line is a `data` field and was recorded.
    ///
    /// Returns false if the line is not a `data` field or `Self::MAX_LINES`
    /// is already reached. The caller must then clear this state and use the
    /// general frame parser.
    fn record_range(&mut self, src: &[u8], end: usize) -> bool {
        if self.values.len() == Self::MAX_LINES {
            return false;
        }

        let line = &src[self.next_line..end];
        let line = line.strip_suffix(&[CR]).unwrap_or(line);
        let Some(value) = field_value(line, b"data") else {
            return false;
        };

        if !self.values.is_empty() {
            // Reserve one byte for the LF separator between joined data lines.
            self.len += 1;
        }

        let value_end = self.next_line + line.len();
        self.len += value.len();
        self.values.push(value_end - value.len()..value_end);
        self.next_line = end + 1;

        true
    }

    /// Joins the recorded `data` lines into a `ByteString`, consumes `consumed`
    /// bytes from `src`, and clears this state.
    ///
    /// Large payloads are compacted in place inside a split slice of `src` to
    /// avoid allocating a new payload buffer.
    fn finish(
        &mut self,
        src: &mut BytesMut,
        consumed: usize,
    ) -> Result<ByteString, str::Utf8Error> {
        let Some((first, rest)) = self.values.split_first() else {
            unreachable!("DataLines::finish requires at least one recorded line");
        };

        let data = if share_payload(src, self.len) {
            let mut frame = src.split_to(consumed);
            let mut write = first.end;

            // The first value is already in place. Move later values toward it.
            for value in rest {
                frame[write] = LF;
                write += 1;
                frame.copy_within(value.clone(), write);
                write += value.len();
            }

            frame.truncate(write);
            frame.advance(first.start);

            ByteString::try_from(frame)
        } else {
            let mut data = Vec::with_capacity(self.len);
            data.extend_from_slice(&src[first.clone()]);

            for value in rest {
                data.push(LF);
                data.extend_from_slice(&src[value.clone()]);
            }

            src.advance(consumed);

            ByteString::try_from(data)
        };

        self.clear();

        data
    }

    /// Resets the recorded ranges while keeping the `Vec` capacity.
    fn clear(&mut self) {
        self.values.clear();
        self.next_line = 0;
        self.len = 0;
    }
}

const MIN_SHARED_PAYLOAD_SIZE: usize = 1024;

/// Returns true when `size` is large enough to share `src`'s backing buffer.
fn share_payload(src: &BytesMut, size: usize) -> bool {
    size >= MIN_SHARED_PAYLOAD_SIZE && size >= src.capacity().div_ceil(2)
}

/// A single value range in the frame or an owned buffer of values joined with `LF`.
enum FieldLines {
    Single(Range<usize>),
    Joined(Vec<u8>),
}

/// Records a value range in `frame` and joins multiple values with `LF`.
fn append_line(lines: &mut Option<FieldLines>, frame: &[u8], range: Range<usize>) {
    match lines {
        None => *lines = Some(FieldLines::Single(range)),

        Some(FieldLines::Single(first)) => {
            let first = &frame[first.clone()];
            let next = &frame[range];
            let mut joined = Vec::with_capacity(first.len() + 1 + next.len());

            joined.extend_from_slice(first);
            joined.push(LF);
            joined.extend_from_slice(next);

            *lines = Some(FieldLines::Joined(joined));
        }

        Some(FieldLines::Joined(joined)) => {
            joined.push(LF);
            joined.extend_from_slice(&frame[range]);
        }
    }
}

/// Converts the accumulated values to a `ByteString` and returns an error for invalid UTF-8.
fn finish_lines(frame: &[u8], lines: FieldLines) -> Result<ByteString, str::Utf8Error> {
    match lines {
        FieldLines::Single(range) => ByteString::try_from(&frame[range]),
        FieldLines::Joined(joined) => ByteString::try_from(joined),
    }
}

/// Collects the fields of one frame before the decoder emits events.
#[derive(Default)]
struct ParsedFrame {
    data: Option<FieldLines>,
    comments: Option<FieldLines>,
    id: Option<ByteString>,
    event: Option<ByteString>,
    retry: Option<Duration>,
}

impl ParsedFrame {
    /// Parses the lines of a complete frame in order.
    ///
    /// Scans for line endings before `last_line_start` and parses the final line directly.
    fn parse_lines(&mut self, frame: &[u8], last_line_start: usize) -> Result<(), Error> {
        let mut line_start = 0;

        for line_end in memchr2_iter(CR, LF, &frame[..last_line_start]) {
            let line = &frame[line_start..line_end];
            let current_start = line_start;
            line_start = line_end + 1;

            self.parse_line(frame, current_start, line)?;
        }

        let last_line = &frame[last_line_start..];
        let last_line = last_line.strip_suffix(&[CR]).unwrap_or(last_line);
        self.parse_line(frame, last_line_start, last_line)
    }

    /// Parses a line that starts at offset `line_start` in `frame`, without its line ending.
    ///
    /// Validates `id` and `event` as UTF-8 immediately. The caller validates `data` and comments
    /// after collecting all lines.
    fn parse_line(&mut self, frame: &[u8], line_start: usize, line: &[u8]) -> Result<(), Error> {
        match line {
            // Skip empty segments from CRLF.
            [] => {}
            [b'd', ..] => {
                if let Some(value) = field_value(line, b"data") {
                    let value_end = line_start + line.len();
                    append_line(&mut self.data, frame, value_end - value.len()..value_end);
                }
            }
            [b':', value @ ..] => {
                let value = strip_space(value);
                let value_end = line_start + line.len();
                append_line(
                    &mut self.comments,
                    frame,
                    value_end - value.len()..value_end,
                );
            }
            [b'i', ..] => {
                if let Some(id) = parse_id(line)? {
                    self.id = Some(id);
                }
            }
            [b'e', ..] => {
                if let Some(value) = field_value(line, b"event") {
                    self.event = Some(ByteString::try_from(value)?);
                }
            }
            [b'r', ..] => {
                if let Some(delay) = field_value(line, b"retry").and_then(parse_retry) {
                    self.retry = Some(delay);
                }
            }
            _ => {}
        }

        Ok(())
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

                if src[0] == LF {
                    src.advance(1);
                }
            }

            let Some(delimiter) = self.find_frame_delimiter(src) else {
                tracing::trace!(buffered_bytes = src.len(), "Not enough data in buffer");
                return Ok(None);
            };

            let mut consumed = delimiter + 2;

            if src[delimiter + 1] == CR {
                match src.get(consumed) {
                    Some(&LF) => consumed += 1,
                    Some(_) => {}
                    None => self.skip_lf = true,
                }
            }

            let kind = mem::take(&mut self.frame_kind);
            let last_line_start = mem::take(&mut self.last_line_start);

            match kind {
                FrameKind::DataLines => {
                    // Record the final line before the frame delimiter.
                    if self.data_lines.record_range(src, delimiter) {
                        let data = self.data_lines.finish(src, consumed)?;

                        return Ok(Some(data_event(data)));
                    }

                    self.data_lines.clear();
                }

                FrameKind::SingleLine => {
                    let frame = &src[..delimiter];
                    // The frame delimiter can start at the LF of a CRLF ending.
                    let line = frame.strip_suffix(&[CR]).unwrap_or(frame);

                    match line {
                        [] => {
                            src.advance(consumed);
                            continue;
                        }

                        [b'd', ..] => {
                            if let Some(value) = line.strip_prefix(b"data:") {
                                let (offset, value) = match value.strip_prefix(b" ") {
                                    Some(value) => (b"data: ".len(), value),
                                    None => (b"data:".len(), value),
                                };
                                let size = value.len();

                                let data = if share_payload(src, size) {
                                    let mut payload = src.split_to(consumed);
                                    payload.advance(offset);
                                    payload.truncate(size);
                                    ByteString::try_from(payload)
                                } else {
                                    let data = ByteString::try_from(value);
                                    src.advance(consumed);
                                    data
                                };

                                return Ok(Some(data_event(data?)));
                            }

                            let is_bare_data = line == b"data";
                            src.advance(consumed);

                            if is_bare_data {
                                return Ok(Some(data_event(ByteString::new())));
                            }

                            continue;
                        }

                        [b':', value @ ..] => {
                            let comment = ByteString::try_from(strip_space(value));
                            src.advance(consumed);

                            return Ok(Some(Event::Comment(comment?)));
                        }

                        [b'r', ..] => {
                            let retry = field_value(line, b"retry").and_then(parse_retry);
                            src.advance(consumed);

                            if let Some(delay) = retry {
                                return Ok(Some(Event::Retry(delay)));
                            }

                            continue;
                        }

                        [b'i', ..] => {
                            let id = parse_id(line);
                            src.advance(consumed);

                            if let Some(id) = id? {
                                return Ok(Some(Event::Message(Message {
                                    data: ByteString::new(),
                                    event: None,
                                    retry: None,
                                    id: Some(id),
                                })));
                            }

                            continue;
                        }

                        [b'e', ..] => {
                            if let Some(value) = field_value(line, b"event") {
                                let event = ByteString::try_from(value);
                                src.advance(consumed);

                                return Ok(Some(Event::Message(Message {
                                    data: ByteString::new(),
                                    event: Some(event?),
                                    retry: None,
                                    id: None,
                                })));
                            }

                            src.advance(consumed);
                            continue;
                        }

                        _ => {
                            src.advance(consumed);
                            continue;
                        }
                    }
                }

                FrameKind::Other => {}
            }

            if let Some(event) = self.parse_frame(src, delimiter, consumed, last_line_start)? {
                return Ok(Some(event));
            }
        }
    }
}

impl Decoder {
    /// Scans `src` for the next blank-line frame delimiter and classifies the
    /// frame while tracking line starts and recording `data`-only lines.
    ///
    /// Returns the index of the first delimiter byte, or `None` if more bytes
    /// are needed.
    fn find_frame_delimiter(&mut self, src: &[u8]) -> Option<usize> {
        // Stop one byte early so the lookahead byte at pos + 1 is always in bounds.
        let end = src.len().saturating_sub(1);
        let mut start = if self.scan_from <= end {
            self.scan_from
        } else {
            // Reset scan state if the caller replaced or truncated src.
            self.frame_kind = FrameKind::SingleLine;
            self.data_lines.clear();
            self.last_line_start = 0;
            0
        };

        while let Some(pos) = memchr2(CR, LF, &src[start..end]) {
            let mut pos = start + pos;
            let mut next = src[pos + 1];

            // CRLF is one line ending; advance to its LF and inspect the next byte directly.
            if src[pos] == CR && next == LF {
                pos += 1;

                if pos == end {
                    self.scan_from = end;
                    return None;
                }

                next = src[pos + 1];
            }

            start = pos + 1;

            match next {
                // CRCR, LFCR, and LFLF end the frame.
                CR | LF => {
                    self.scan_from = 0;
                    return Some(pos);
                }

                // A non-ending byte starts another line; record the line that just ended.
                _ => {
                    self.last_line_start = pos + 1;

                    if !matches!(self.frame_kind, FrameKind::Other) {
                        self.frame_kind = if self.data_lines.record_range(src, pos) {
                            FrameKind::DataLines
                        } else {
                            self.data_lines.clear();
                            FrameKind::Other
                        };
                    }
                }
            }
        }

        // Keep one byte of overlap for delimiters split across reads.
        self.scan_from = end;
        None
    }

    /// Parses the frame in `src[..delimiter]` and consumes `consumed` bytes, even on error.
    ///
    /// Uses `last_line_start` to skip scanning the final line for a line ending. A large
    /// single-line `data` value can share the backing buffer of `src`.
    fn parse_frame(
        &mut self,
        src: &mut BytesMut,
        delimiter: usize,
        consumed: usize,
        last_line_start: usize,
    ) -> Result<Option<Event>, Error> {
        let mut parsed = ParsedFrame::default();

        if let Err(err) = parsed.parse_lines(&src[..delimiter], last_line_start) {
            src.advance(consumed);
            return Err(err);
        }

        let (data, comment) = match parsed.data {
            Some(FieldLines::Single(range)) if share_payload(src, range.len()) => {
                let mut payload = src.split_to(consumed);
                let comment = parsed.comments.map(|lines| finish_lines(&payload, lines));

                payload.advance(range.start);
                payload.truncate(range.len());

                let data = Some(ByteString::try_from(payload)?);
                let comment = comment.transpose()?;

                (data, comment)
            }

            data => {
                let frame = &src[..delimiter];
                let data = data.map(|lines| finish_lines(frame, lines)).transpose();

                let comment = match &data {
                    Err(_) => Ok(None),
                    Ok(_) => parsed
                        .comments
                        .map(|lines| finish_lines(frame, lines))
                        .transpose(),
                };

                src.advance(consumed);

                (data?, comment?)
            }
        };

        let event = if data.is_some() || parsed.id.is_some() || parsed.event.is_some() {
            Some(Event::Message(Message {
                data: data.unwrap_or_default(),
                event: parsed.event,
                retry: parsed.retry,
                id: parsed.id,
            }))
        } else {
            parsed.retry.map(Event::Retry)
        };

        if let Some(comment) = comment {
            self.pending_event = event;
            Ok(Some(Event::Comment(comment)))
        } else {
            Ok(event)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error as _, io, pin::pin, time::Duration};

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

    const SSE_DELIMITER: &[u8] = b"\n\n";

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
    fn ignores_single_line_id_containing_nul() {
        let mut input = BytesMut::from("id: a\0b\n\ndata: next\n\n");

        assert_eq!(
            Decoder::default().decode(&mut input).unwrap(),
            Some(Event::Message(Message::data("next"))),
        );
    }

    #[test]
    fn ignores_id_containing_nul_between_data_lines() {
        let mut input = BytesMut::from("id: keep\ndata: first\nid: a\0b\ndata: last\n\n");

        assert_eq!(
            Decoder::default().decode(&mut input).unwrap(),
            Some(Event::Message(Message {
                id: Some("keep".into()),
                ..Message::data("first\nlast")
            })),
        );
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
    fn preserves_empty_comment_lines() {
        for (input, expected) in [
            (":\n\n", ""),
            (":\n: second\n\n", "\nsecond"),
            (": first\n:\n\n", "first\n"),
        ] {
            let mut input = BytesMut::from(input);

            let event = Decoder::default().decode(&mut input).unwrap();

            assert_eq!(event, Some(Event::Comment(expected.into())));
        }
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
        for (input, expected) in [
            ("data: hello\r\n\r\n", "hello"),
            ("data:  left:right\r\n\r\n", " left:right"),
            ("data:\r\n\r\n", ""),
            (
                "data: first\r\ndata: second\r\ndata: third\r\n\r\n",
                "first\nsecond\nthird",
            ),
            ("data: \r\ndata: second\r\n\r\n", "\nsecond"),
        ] {
            let mut input = BytesMut::from(input);
            let mut decoder = Decoder::default();

            let event = decoder.decode(&mut input).unwrap();

            assert_eq!(event, Some(Event::Message(Message::data(expected))));
            assert!(input.is_empty());
            assert!(!decoder.skip_lf);
        }
    }

    #[test]
    fn decodes_crlf_split_across_chunk_boundaries() {
        // Chunk ending right after `\r` in both line and frame endings.
        for chunks in [
            &["data: first\r", "\ndata: second\r", "\n\r", "\n"][..],
            &["data: first\r\n", "data: second\r\n", "\r\n"][..],
            &["data: first\r\ndata: second\r\n\r", "\n"][..],
        ] {
            let mut decoder = Decoder::default();
            let mut buffer = BytesMut::new();
            let mut events = Vec::new();

            for chunk in chunks {
                buffer.extend_from_slice(chunk.as_bytes());

                while let Some(event) = decoder.decode(&mut buffer).unwrap() {
                    events.push(event);
                }
            }

            assert_eq!(
                events,
                [Event::Message(Message::data("first\nsecond"))],
                "chunks: {chunks:?}",
            );
            assert!(buffer.is_empty(), "chunks: {chunks:?}");
            assert!(!decoder.skip_lf, "chunks: {chunks:?}");
        }

        // Chunk ending right after `\r\n\r` sets `skip_lf` and consumes the trailing `\n`.
        let mut decoder = Decoder::default();
        let mut buffer = BytesMut::from("data: first\r\n\r");

        assert_eq!(
            decoder.decode(&mut buffer).unwrap(),
            Some(Event::Message(Message::data("first"))),
        );
        assert!(buffer.is_empty());
        assert!(decoder.skip_lf);

        buffer.extend_from_slice(b"\ndata: second\r\n\r\n");

        assert_eq!(
            decoder.decode(&mut buffer).unwrap(),
            Some(Event::Message(Message::data("second"))),
        );
        assert!(buffer.is_empty());
        assert!(!decoder.skip_lf);
    }

    #[test]
    fn decodes_empty_crlf_and_mixed_cr_delimiters() {
        // Standalone empty CRLF frame.
        let mut decoder = Decoder::default();
        let mut buffer = BytesMut::from("\r\n\r\n");

        assert_none!(decoder.decode(&mut buffer).unwrap());
        assert!(buffer.is_empty());
        assert!(!decoder.skip_lf);

        // Empty CRLF frame followed by a message.
        let mut decoder = Decoder::default();
        let mut buffer = BytesMut::from("\r\n\r\ndata: after\r\n\r\n");

        assert_eq!(
            decoder.decode(&mut buffer).unwrap(),
            Some(Event::Message(Message::data("after"))),
        );
        assert!(buffer.is_empty());
        assert!(!decoder.skip_lf);

        // `\r\r\n` delimiter after a line and after an empty frame.
        let mut decoder = Decoder::default();
        let mut buffer = BytesMut::from("\r\r\ndata: hello\r\r\ndata: next\r\n\r\n");

        assert_eq!(
            decoder.decode(&mut buffer).unwrap(),
            Some(Event::Message(Message::data("hello"))),
        );
        assert!(!decoder.skip_lf);
        assert_eq!(
            decoder.decode(&mut buffer).unwrap(),
            Some(Event::Message(Message::data("next"))),
        );
        assert!(buffer.is_empty());
        assert!(!decoder.skip_lf);

        // CR delimiter followed immediately by a non-LF byte clears `skip_lf` eagerly.
        for input in [
            "data: first\r\rdata: second\r\r",
            "data: first\r\n\rdata: second\r\n\r\n",
        ] {
            let mut decoder = Decoder::default();
            let mut buffer = BytesMut::from(input);

            assert_eq!(
                decoder.decode(&mut buffer).unwrap(),
                Some(Event::Message(Message::data("first"))),
            );
            assert!(!decoder.skip_lf, "input: {input:?}");

            assert_eq!(
                decoder.decode(&mut buffer).unwrap(),
                Some(Event::Message(Message::data("second"))),
            );
            assert!(buffer.is_empty(), "input: {input:?}");
        }
    }

    #[test]
    fn decodes_frames_across_chunk_boundaries() {
        let input = "data: café\r\ndata: 世界\r\r\n: keepalive\n\r\ndata: last\n\n";
        let input = input.as_bytes();
        let expected = [
            Event::Message(Message::data("café\n世界")),
            Event::Comment("keepalive".into()),
            Event::Message(Message::data("last")),
        ];

        for chunk_size in 1..=input.len() {
            let mut decoder = Decoder::default();
            let mut buffer = BytesMut::new();
            let mut events = Vec::new();

            for chunk in input.chunks(chunk_size) {
                buffer.extend_from_slice(chunk);

                while let Some(event) = decoder.decode(&mut buffer).unwrap() {
                    events.push(event);
                }
            }

            assert_eq!(events, expected, "chunk size {chunk_size}");
            assert!(buffer.is_empty(), "chunk size {chunk_size}");
        }
    }

    #[test]
    fn invalid_fragmented_frame_leaves_following_frames_in_buffer() {
        for delimiter in [b"\n\n".as_slice(), b"\r\n\r\n"] {
            let mut decoder = Decoder::default();
            let mut buffer = BytesMut::from(&b"data: invalid UTF-8 \xff"[..]);

            assert_none!(decoder.decode(&mut buffer).unwrap());

            let following = b"data: next\n\ndata: another message\n\n";
            buffer.extend_from_slice(delimiter);
            buffer.extend_from_slice(following);

            assert!(matches!(
                decoder.decode(&mut buffer),
                Err(Error::InvalidUtf8 { .. })
            ));
            assert_eq!(&buffer[..], following);
            assert_eq!(
                decoder.decode(&mut buffer).unwrap(),
                Some(Event::Message(Message::data("next"))),
            );
        }
    }

    #[test]
    fn resets_scan_state_when_input_is_replaced_or_truncated() {
        let mut decoder = Decoder::default();
        let mut partial = BytesMut::from("data: first\ndata: second");

        assert_none!(decoder.decode(&mut partial).unwrap());

        // Replace with a shorter single-line frame so scan_from exceeds the new buffer length.
        let mut replaced = BytesMut::from("data: ok\n\n");

        assert_eq!(
            decoder.decode(&mut replaced).unwrap(),
            Some(Event::Message(Message::data("ok"))),
        );
        assert!(replaced.is_empty());

        // Repeat with an in-place clear and a shorter multiline frame.
        let mut buffer = BytesMut::from("data: first\ndata: second\ndata: third");

        assert_none!(decoder.decode(&mut buffer).unwrap());

        buffer.clear();
        buffer.extend_from_slice(b"data: a\ndata: b\n\n");

        assert_eq!(
            decoder.decode(&mut buffer).unwrap(),
            Some(Event::Message(Message::data("a\nb"))),
        );
        assert!(buffer.is_empty());

        // Repeat when the buffer is cleared to empty before the next read arrives.
        let mut buffer = BytesMut::from("data: first\ndata: second");

        assert_none!(decoder.decode(&mut buffer).unwrap());

        buffer.clear();
        assert_none!(decoder.decode(&mut buffer).unwrap());

        buffer.extend_from_slice(b"data: after clear\n\n");

        assert_eq!(
            decoder.decode(&mut buffer).unwrap(),
            Some(Event::Message(Message::data("after clear"))),
        );
        assert!(buffer.is_empty());

        // Repeat with a multi-field partial frame so last_line_start also resets.
        let mut buffer = BytesMut::from("event: first\ndata: second");

        assert_none!(decoder.decode(&mut buffer).unwrap());

        buffer.clear();
        buffer.extend_from_slice(b"event: ok\n\n");

        assert_eq!(
            decoder.decode(&mut buffer).unwrap(),
            Some(Event::Message(Message {
                event: Some("ok".into()),
                ..Default::default()
            })),
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn shares_large_single_line_data_and_retains_it_when_input_grows() {
        let data = "café".repeat(256);
        let frame = format!("data: {data}\r\n\r\n");
        let mut input = BytesMut::from(frame.as_str());
        let payload = input[b"data: ".len()..].as_ptr();
        let mut decoder = Decoder::default();

        let Some(Event::Message(first)) = decoder.decode(&mut input).unwrap() else {
            panic!("expected a message");
        };

        assert_eq!(first.data.as_ptr(), payload);

        input.extend_from_slice(frame.as_bytes());

        assert_eq!(
            decoder.decode(&mut input).unwrap(),
            Some(Event::Message(Message::data(data.as_str()))),
        );
        assert_eq!(&*first.data, data);
        assert!(input.is_empty());
    }

    #[test]
    fn shares_large_single_line_data_in_multi_field_frames() {
        let data = "café".repeat(256);

        for (prefix, suffix, expected_event, expected_id) in [
            ("event: update\n", "", Some("update"), None),
            ("id: 42\r\n", "", None, Some("42")),
            ("", "\nevent: after", Some("after"), None),
            ("id: 7\nevent: update\n", "", Some("update"), Some("7")),
        ] {
            let frame = format!("{prefix}data: {data}{suffix}\r\n\r\n");
            let mut input = BytesMut::from(frame.as_str());
            let payload_offset = prefix.len() + b"data: ".len();
            let payload = input[payload_offset..].as_ptr();
            let mut decoder = Decoder::default();

            let Some(Event::Message(first)) = decoder.decode(&mut input).unwrap() else {
                panic!("expected a message");
            };

            assert_eq!(first.data.as_ptr(), payload);
            assert_eq!(first.event.as_deref(), expected_event);
            assert_eq!(first.id.as_deref(), expected_id);

            input.extend_from_slice(frame.as_bytes());

            assert_eq!(
                decoder.decode(&mut input).unwrap(),
                Some(Event::Message(Message {
                    event: expected_event.map(ByteString::from),
                    id: expected_id.map(ByteString::from),
                    ..Message::data(data.as_str())
                })),
            );
            assert_eq!(&*first.data, data);
            assert!(input.is_empty());
        }
    }

    #[test]
    fn shares_large_single_line_data_with_comments() {
        let data = "café".repeat(256);

        for (frame, payload_offset, expected_comment, expected_event) in [
            (
                format!(": heartbeat\nevent: update\ndata: {data}\r\n\r\n"),
                b": heartbeat\nevent: update\ndata: ".len(),
                "heartbeat",
                Some("update"),
            ),
            (
                format!(": first\n: second\ndata: {data}\n: third\n\n"),
                b": first\n: second\ndata: ".len(),
                "first\nsecond\nthird",
                None,
            ),
        ] {
            let mut input = BytesMut::from(frame.as_str());
            let payload = input[payload_offset..].as_ptr();
            let mut decoder = Decoder::default();

            assert_eq!(
                decoder.decode(&mut input).unwrap(),
                Some(Event::Comment(expected_comment.into())),
            );

            let Some(Event::Message(message)) = decoder.decode(&mut input).unwrap() else {
                panic!("expected a message");
            };

            assert_eq!(message.data.as_ptr(), payload);
            assert_eq!(&*message.data, data);
            assert_eq!(message.event.as_deref(), expected_event);
            assert!(input.is_empty());
        }
    }

    #[test]
    fn invalid_utf8_in_multi_field_frames_consumes_frame_and_preserves_precedence() {
        let large_prefix = "a".repeat(2048);

        let mut large_bad_data = Vec::from(b"event: update\ndata: ".as_slice());
        large_bad_data.extend_from_slice(large_prefix.as_bytes());
        large_bad_data.push(0xff);

        let mut large_bad_comment = Vec::from(b": bad\xff\nevent: update\ndata: ".as_slice());
        large_bad_comment.extend_from_slice(large_prefix.as_bytes());

        let mut both_bad = Vec::from(b": \xff\nevent: update\ndata: ".as_slice());
        both_bad.extend_from_slice(large_prefix.as_bytes());
        both_bad.extend_from_slice(b"\xC3\x28");

        let cases: &[(&[u8], usize)] = &[
            (b"id: bad\xff\ndata: ok", 3),
            (b"event: bad\xff\ndata: ok", 3),
            (b"event: update\ndata: bad\xff", 3),
            (b": bad\xff\nevent: update\ndata: small", 3),
            (&large_bad_data, large_prefix.len()),
            (&large_bad_comment, 3),
            (&both_bad, large_prefix.len()),
        ];

        for delimiter in [b"\n\n".as_slice(), b"\r\n\r\n"] {
            for &(frame, expected_valid_up_to) in cases {
                let mut decoder = Decoder::default();
                let mut buffer = BytesMut::from(frame);

                let following = b"data: next\n\n";
                buffer.extend_from_slice(delimiter);
                buffer.extend_from_slice(following);

                let err = decoder.decode(&mut buffer).unwrap_err();
                let Error::InvalidUtf8 { source } = err else {
                    panic!("expected invalid UTF-8 error, got: {err:?}");
                };

                assert_eq!(source.valid_up_to(), expected_valid_up_to);
                assert_eq!(&buffer[..], following);
                assert_eq!(
                    decoder.decode(&mut buffer).unwrap(),
                    Some(Event::Message(Message::data("next"))),
                );
                assert!(buffer.is_empty());
            }
        }
    }

    #[test]
    fn decodes_multi_field_frames_across_all_chunk_sizes() {
        let input = concat!(
            "event: update\r\n",
            "id: 42\r\n",
            ": heartbeat\r\n",
            "retry: 500\r\n",
            "data: café\r\n\r\n",
            "id: 43\n",
            "data: hello\n\n",
            "event: ping\r",
            "data: world\r\r",
        );
        let input = input.as_bytes();
        let expected = [
            Event::Comment("heartbeat".into()),
            Event::Message(Message {
                data: "café".into(),
                event: Some("update".into()),
                id: Some("42".into()),
                retry: Some(Duration::from_millis(500)),
            }),
            Event::Message(Message {
                data: "hello".into(),
                id: Some("43".into()),
                ..Default::default()
            }),
            Event::Message(Message {
                data: "world".into(),
                event: Some("ping".into()),
                ..Default::default()
            }),
        ];

        for chunk_size in 1..=input.len() {
            let mut decoder = Decoder::default();
            let mut buffer = BytesMut::new();
            let mut events = Vec::new();

            for chunk in input.chunks(chunk_size) {
                buffer.extend_from_slice(chunk);

                while let Some(event) = decoder.decode(&mut buffer).unwrap() {
                    events.push(event);
                }
            }

            assert_eq!(events, expected, "chunk size {chunk_size}");
            assert!(buffer.is_empty(), "chunk size {chunk_size}");
        }
    }

    #[test]
    fn retains_fragmented_multiline_data_when_the_read_buffer_grows() {
        let left = "café".repeat(512);
        let right = "世界".repeat(512);
        let frame = format!("data: {left}\r\ndata\rdata:  {right}\n\r\n");
        let expected = Event::Message(Message::data(format!("{left}\n\n {right}")));
        let expected = [expected.clone(), expected];

        for chunk_size in [1, 7, 1460, frame.len()] {
            let mut decoder = Decoder::default();
            let mut input = BytesMut::new();
            let mut events = Vec::new();

            for _ in 0..2 {
                for chunk in frame.as_bytes().chunks(chunk_size) {
                    input.extend_from_slice(chunk);
                    while let Some(event) = decoder.decode(&mut input).unwrap() {
                        events.push(event);
                    }
                }
            }

            assert!(input.is_empty());
            assert_eq!(events, expected, "chunk size {chunk_size}");
        }
    }

    #[test]
    fn preserves_large_multiline_data_with_fields_after_the_data() {
        let data = "x".repeat(4096);
        let frame = format!("data: {data}\ndata: tail\nid: 42\n: heartbeat\n\n");
        let mut decoder = Decoder::default();
        let mut input = BytesMut::from(frame.as_str());

        assert_eq!(
            decoder.decode(&mut input).unwrap(),
            Some(Event::Comment("heartbeat".into())),
        );
        assert_eq!(
            decoder.decode(&mut input).unwrap(),
            Some(Event::Message(Message {
                id: Some("42".into()),
                ..Message::data(format!("{data}\ntail"))
            })),
        );
        assert!(input.is_empty());
    }

    #[test]
    fn decodes_more_than_the_maximum_tracked_data_lines() {
        let lines = DataLines::MAX_LINES + 1;
        let frame = format!("{}\ndata: next\n\n", "data: x\n".repeat(lines));
        let data = "x\n".repeat(lines - 1) + "x";
        let mut decoder = Decoder::default();
        let mut input = BytesMut::from(frame.as_str());

        assert_eq!(
            decoder.decode(&mut input).unwrap(),
            Some(Event::Message(Message::data(data))),
        );
        assert_eq!(
            decoder.decode(&mut input).unwrap(),
            Some(Event::Message(Message::data("next"))),
        );
        assert!(input.is_empty());
    }

    #[test]
    fn decodes_many_multiline_comments() {
        for ending in ["\n", "\r\n"] {
            for count in [2, 64] {
                let mut frame = String::new();
                let mut expected = Vec::with_capacity(count);

                for i in 0..count {
                    let text = if i % 7 == 0 {
                        String::new()
                    } else {
                        format!("comment-{i}")
                    };
                    frame.push_str(": ");
                    frame.push_str(&text);
                    frame.push_str(ending);
                    expected.push(text);
                }

                frame.push_str(ending);

                let mut decoder = Decoder::default();
                let mut input = BytesMut::from(frame.as_str());

                assert_eq!(
                    decoder.decode(&mut input).unwrap(),
                    Some(Event::Comment(expected.join("\n").into())),
                );
                assert!(input.is_empty());
            }
        }
    }

    #[test]
    fn decodes_mixed_field_multiline_data() {
        for ending in ["\n", "\r\n"] {
            for count in [2, 64] {
                let mut frame = format!(
                    "id: 42{ending}event: update{ending}retry: 250{ending}: note-1{ending}: note-2{ending}: note-3{ending}"
                );
                let mut expected_data = Vec::with_capacity(count);

                for i in 0..count {
                    let line = format!("chunk-{i}");
                    frame.push_str("data: ");
                    frame.push_str(&line);
                    frame.push_str(ending);
                    expected_data.push(line);
                }

                frame.push_str(ending);

                let mut decoder = Decoder::default();
                let mut input = BytesMut::from(frame.as_str());

                assert_eq!(
                    decoder.decode(&mut input).unwrap(),
                    Some(Event::Comment("note-1\nnote-2\nnote-3".into())),
                );
                assert_eq!(
                    decoder.decode(&mut input).unwrap(),
                    Some(Event::Message(Message {
                        data: expected_data.join("\n").into(),
                        id: Some("42".into()),
                        event: Some("update".into()),
                        retry: Some(Duration::from_millis(250)),
                    })),
                );
                assert!(input.is_empty());
            }
        }
    }

    #[test]
    fn decodes_more_than_max_tracked_data_lines_with_crlf_and_varied_lengths() {
        let count = DataLines::MAX_LINES + 128;
        let mut frame = String::new();
        let mut expected = Vec::with_capacity(count);

        for i in 0..count {
            let line = format!("row-{i}-{}", "a".repeat(i % 17));
            frame.push_str("data: ");
            frame.push_str(&line);
            frame.push_str("\r\n");
            expected.push(line);
        }

        frame.push_str("\r\n");

        let mut decoder = Decoder::default();
        let mut input = BytesMut::from(frame.as_str());

        assert_eq!(
            decoder.decode(&mut input).unwrap(),
            Some(Event::Message(Message::data(expected.join("\n")))),
        );
        assert!(input.is_empty());
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
                database: ignored
                identifier: ignored
                eventual: ignored
                retrying: 9000
                data: hello

            "},
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

    #[test]
    fn preserves_extra_space_and_colons_in_field_values() {
        let mut input = BytesMut::from("data:  left:right\nid:  café:42\n\n");

        let event = Decoder::default().decode(&mut input).unwrap();

        assert_eq!(
            event,
            Some(Event::Message(Message {
                id: Some(" café:42".into()),
                ..Message::data(" left:right")
            })),
        );
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

    #[test]
    fn decodes_single_line_retry_frames() {
        for (input, expected) in [
            (
                "retry: 42\n\n",
                Some(Event::Retry(Duration::from_millis(42))),
            ),
            (
                "retry:42\r\n\r\n",
                Some(Event::Retry(Duration::from_millis(42))),
            ),
            (
                "retry: 18446744073709551616\n\n",
                Some(Event::Retry(Duration::new(
                    18_446_744_073_709_551,
                    616_000_000,
                ))),
            ),
            ("retry: invalid\n\n", None),
            ("retry\n\n", None),
            ("retrying: 42\n\n", None),
        ] {
            let mut input = BytesMut::from(input);

            assert_eq!(Decoder::default().decode(&mut input).unwrap(), expected);
            assert!(input.is_empty());
        }
    }

    #[test]
    fn decodes_single_line_id_and_event_frames() {
        for (input, expected) in [
            (
                "id: 42\n\n",
                Some(Event::Message(Message {
                    id: Some("42".into()),
                    ..Default::default()
                })),
            ),
            (
                "id:42\r\n\r\n",
                Some(Event::Message(Message {
                    id: Some("42".into()),
                    ..Default::default()
                })),
            ),
            (
                "id\n\n",
                Some(Event::Message(Message {
                    id: Some("".into()),
                    ..Default::default()
                })),
            ),
            ("identifier: 42\n\n", None),
            (
                "event: ping\n\n",
                Some(Event::Message(Message {
                    event: Some("ping".into()),
                    ..Default::default()
                })),
            ),
            (
                "event:ping\r\n\r\n",
                Some(Event::Message(Message {
                    event: Some("ping".into()),
                    ..Default::default()
                })),
            ),
            (
                "event\n\n",
                Some(Event::Message(Message {
                    event: Some("".into()),
                    ..Default::default()
                })),
            ),
            ("eventual: ping\n\n", None),
        ] {
            let mut input = BytesMut::from(input);

            assert_eq!(Decoder::default().decode(&mut input).unwrap(), expected);
            assert!(input.is_empty());
        }
    }

    #[test]
    fn decodes_single_line_bare_data_and_unknown_d_fields() {
        let mut input = BytesMut::from("data\n\n");
        assert_eq!(
            Decoder::default().decode(&mut input).unwrap(),
            Some(Event::Message(Message::data(""))),
        );
        assert!(input.is_empty());

        let mut input = BytesMut::from("data\r\n\r\n");
        assert_eq!(
            Decoder::default().decode(&mut input).unwrap(),
            Some(Event::Message(Message::data(""))),
        );
        assert!(input.is_empty());

        for ignored in ["database: ignored\n\n", "data2: ignored\r\n\r\n", "d\n\n"] {
            let mut input = BytesMut::from(format!("{ignored}data: next\n\n").as_str());

            assert_eq!(
                Decoder::default().decode(&mut input).unwrap(),
                Some(Event::Message(Message::data("next"))),
            );
            assert!(input.is_empty());
        }
    }

    #[test]
    fn skips_single_line_empty_frames() {
        for empty in ["\n\n", "\r\n\r\n", "\r\r", "\n\r", "\n\n\r\n\r\n"] {
            let mut input = BytesMut::from(empty);

            assert_none!(Decoder::default().decode(&mut input).unwrap());
            assert!(input.is_empty(), "empty frame {empty:?} was not consumed");

            let mut input = BytesMut::from(format!("{empty}data: ok\n\n").as_str());

            assert_eq!(
                Decoder::default().decode(&mut input).unwrap(),
                Some(Event::Message(Message::data("ok"))),
            );
            assert!(input.is_empty());
        }
    }

    #[test]
    fn consumes_invalid_utf8_single_line_frames() {
        let large_invalid = [
            b"data: ".as_slice(),
            &vec![b'x'; MIN_SHARED_PAYLOAD_SIZE],
            b"\xff",
        ]
        .concat();

        let cases: Vec<&[u8]> = vec![
            b"data: \xff",
            &large_invalid,
            b": \xff",
            b"id: \xff",
            b"event: \xff",
        ];

        for frame in cases {
            for delimiter in [b"\n\n".as_slice(), b"\r\n\r\n"] {
                let mut decoder = Decoder::default();
                let mut buffer = BytesMut::from(frame);
                let following = b"data: next\n\n";

                buffer.extend_from_slice(delimiter);
                buffer.extend_from_slice(following);

                assert!(matches!(
                    decoder.decode(&mut buffer),
                    Err(Error::InvalidUtf8 { .. })
                ));
                assert_eq!(&buffer[..], following);
                assert_eq!(
                    decoder.decode(&mut buffer).unwrap(),
                    Some(Event::Message(Message::data("next"))),
                );
                assert!(buffer.is_empty());
            }
        }
    }
}
