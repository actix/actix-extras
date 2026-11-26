use std::{num::NonZeroU32, ops::Range, str, time::Duration};

use bytes::{Buf as _, BytesMut};
use bytestring::ByteString;
use memchr::{memchr, memchr2};

use crate::{
    CR, Error, LF,
    decoder::{
        fields::{field_value, parse_id_value, parse_retry},
        share_payload,
    },
    event::Event,
    message::Message,
};

#[derive(Debug, Clone, Copy)]
struct ByteRange {
    start: u32,
    end: NonZeroU32,
}

impl ByteRange {
    /// Creates a byte range for a value at the end of a line starting at `line_start`.
    fn from_suffix(line_start: usize, line_len: usize, value_len: usize) -> Option<Self> {
        let value_end = line_start + line_len;
        let end = NonZeroU32::new(u32::try_from(value_end).ok()?)?;

        Some(Self {
            start: (value_end - value_len) as u32,
            end,
        })
    }

    fn as_range(self) -> Range<usize> {
        self.start as usize..self.end.get() as usize
    }

    fn len(self) -> usize {
        (self.end.get() - self.start) as usize
    }
}

/// Returns the start offset of the value for a known `data` line starting at `line_start`.
fn data_value_start(src: &[u8], line_start: usize, limit: usize) -> usize {
    let mut value_start = line_start + b"data".len();

    if value_start < limit && src[value_start] == b':' {
        value_start += 1;

        if value_start < limit && src[value_start] == b' ' {
            value_start += 1;
        }
    }

    value_start
}

/// Fast-path field offsets for a frame with a contiguous block of `data` lines.
///
/// Tracks optional `event`, `id`, and `retry` fields alongside the `data` block using `u32` byte
/// offsets into the decoder's input buffer.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MessageFrame {
    /// Byte range of the `event` field value in `src`, if seen.
    event: Option<ByteRange>,

    /// Byte range of the `id` field value in `src`, if seen.
    id: Option<ByteRange>,

    /// Last valid `retry` delay in this frame, if seen.
    retry: Option<Duration>,

    /// Byte ranges of the first `data` field values in `src`, avoiding a re-scan for short
    /// multiline payloads.
    ///
    /// Only the first `min(data_count, Self::INLINE_LINES)` entries are valid; `clear` does not
    /// reset this array.
    inline_data: [Option<ByteRange>; Self::INLINE_LINES],

    /// Exclusive end byte offset in `src` of the last recorded `data` value, before its line
    /// terminator.
    data_end: u32,

    /// Exact byte length of the joined `data` payload, including `LF` separators between
    /// consecutive `data` lines.
    data_len: u32,

    /// Number of `data` lines recorded in this frame.
    data_count: u32,

    /// Start byte offset in `src` of the next line to record.
    next_line: u32,

    /// Set once a non-`data` field appears after at least one `data` line.
    ///
    /// Prevents later `data` lines from entering this fast path so the recorded `data` lines stay
    /// contiguous.
    data_closed: bool,

    /// Set if any recorded line ended with `CR` or `CRLF`.
    ///
    /// When `false`, the multiline `data` scan searches only for `LF`.
    has_cr: bool,
}

impl MessageFrame {
    // Store short multiline payloads inline to avoid re-scanning data lines.
    const INLINE_LINES: usize = 2;

    /// Records the next line in `src`, where `end` is the index of its line terminator.
    ///
    /// Accepts a contiguous block of `data` lines with optional `event`, `id`, and `retry` fields
    /// before or after the `data` lines (at most one `event` and one `id`).
    ///
    /// Returns false if the line is a comment, unknown field, duplicate `event` or `id`, a `data`
    /// line after non-`data` fields following earlier `data` lines, or exceeds `u32` offsets. The
    /// caller must then clear this state and use the general frame parser.
    pub(crate) fn record_line(&mut self, src: &[u8], end: usize) -> bool {
        let Ok(next_line) = u32::try_from(end + 1) else {
            return false;
        };

        let line_start = self.next_line as usize;
        let line = &src[line_start..end];
        let line = match line.strip_suffix(&[CR]) {
            Some(stripped) => {
                self.has_cr = true;
                stripped
            }
            None => {
                self.has_cr |= src[end] == CR;
                line
            }
        };

        match line {
            [b'd', ..] => {
                if self.data_closed {
                    return false;
                }

                let Some(value) = field_value(line, b"data") else {
                    return false;
                };

                self.push_data(line_start, line.len(), value.len(), next_line)
            }

            [b'e', ..] => {
                if self.event.is_some() {
                    return false;
                }

                let Some(value) = field_value(line, b"event") else {
                    return false;
                };

                let Some(range) = ByteRange::from_suffix(line_start, line.len(), value.len())
                else {
                    return false;
                };

                self.event = Some(range);
                self.data_closed |= self.data_count > 0;
                self.next_line = next_line;

                true
            }

            [b'i', ..] => {
                if self.id.is_some() {
                    return false;
                }

                let Some(value) = field_value(line, b"id") else {
                    return false;
                };

                let Some(range) = ByteRange::from_suffix(line_start, line.len(), value.len())
                else {
                    return false;
                };

                self.id = Some(range);
                self.data_closed |= self.data_count > 0;
                self.next_line = next_line;

                true
            }

            [b'r', ..] => {
                let Some(value) = field_value(line, b"retry") else {
                    return false;
                };

                if let Some(delay) = parse_retry(value) {
                    self.retry = Some(delay);
                }

                self.data_closed |= self.data_count > 0;
                self.next_line = next_line;

                true
            }

            _ => false,
        }
    }

    fn push_data(
        &mut self,
        line_start: usize,
        line_len: usize,
        value_len: usize,
        next_line: u32,
    ) -> bool {
        let Some(range) = ByteRange::from_suffix(line_start, line_len, value_len) else {
            return false;
        };

        let index = self.data_count as usize;

        if index < Self::INLINE_LINES {
            self.inline_data[index] = Some(range);
        }

        let extra = value_len as u32 + u32::from(self.data_count > 0);
        let Some(data_len) = self.data_len.checked_add(extra) else {
            return false;
        };

        self.data_end = range.end.get();
        self.data_len = data_len;
        self.data_count += 1;
        self.next_line = next_line;

        true
    }

    /// Builds an event from the recorded fields, consumes `consumed` bytes from `src`, and clears
    /// this state.
    ///
    /// Returns `Ok(None)` when the complete frame produces no event (for example, invalid `retry`
    /// values or an ignored NUL-containing `id` with no `data` or `event`). The frame bytes are
    /// still consumed so the caller can continue to the next frame.
    ///
    /// Large `data` payloads are sliced or compacted in place inside a split slice of `src` to
    /// avoid allocating a new payload buffer.
    #[inline(never)]
    pub(crate) fn finish(
        &mut self,
        src: &mut BytesMut,
        consumed: usize,
    ) -> Result<Option<Event>, Error> {
        let result = self.finish_inner(src, consumed);

        self.clear();

        result
    }

    /// Implements [`Self::finish`] without clearing state.
    ///
    /// Returns `Ok(None)` when the consumed frame produces no event (not when more input is
    /// needed).
    fn finish_inner(&self, src: &mut BytesMut, consumed: usize) -> Result<Option<Event>, Error> {
        // Validate `event` and `id` in line order before `data` to match `ParsedFrame`.
        let (event, id) = match self.extract_headers(src) {
            Ok(headers) => headers,
            Err(err) => {
                src.advance(consumed);
                return Err(err.into());
            }
        };

        if self.data_count == 0 {
            src.advance(consumed);

            if event.is_some() || id.is_some() {
                return Ok(Some(Event::Message(Message {
                    data: ByteString::new(),
                    event,
                    retry: self.retry,
                    id,
                })));
            }

            return Ok(self.retry.map(Event::Retry));
        }

        let Some(first) = self.inline_data[0] else {
            unreachable!("MessageFrame with data_count > 0 must record first line");
        };
        let first_range = first.as_range();
        let data_len = self.data_len as usize;

        let data = if self.data_count == 1 {
            if share_payload(src, first_range.len()) {
                let mut frame = src.split_to(consumed);
                frame.truncate(first_range.end);
                frame.advance(first_range.start);

                ByteString::try_from(frame)
            } else {
                let data = ByteString::try_from(&src[first_range]);
                src.advance(consumed);

                data
            }
        } else if (self.data_count as usize) <= Self::INLINE_LINES {
            let rest_ranges = &self.inline_data[1..self.data_count as usize];

            if share_payload(src, data_len) {
                let mut frame = src.split_to(consumed);
                let mut write_idx = first_range.end;

                // The first value is already in place. Move later values toward it.
                for value in rest_ranges.iter().flatten() {
                    frame[write_idx] = LF;
                    write_idx += 1;
                    frame.copy_within(value.as_range(), write_idx);
                    write_idx += value.len();
                }

                frame.truncate(write_idx);
                frame.advance(first_range.start);

                ByteString::try_from(frame)
            } else {
                let mut data = Vec::with_capacity(data_len);
                data.extend_from_slice(&src[first_range]);

                for value in rest_ranges.iter().flatten() {
                    data.push(LF);
                    data.extend_from_slice(&src[value.as_range()]);
                }

                src.advance(consumed);

                ByteString::try_from(data)
            }
        } else {
            let data_end = self.data_end as usize;
            let mut next_start = first_range.end + 1;

            if self.has_cr && src[first_range.end] == CR && src.get(next_start) == Some(&LF) {
                next_start += 1;
            }

            if share_payload(src, data_len) {
                let mut frame = src.split_to(consumed);
                let mut read_idx = next_start;
                let mut write_idx = first_range.end;

                // The first value is already in place. Compact later values toward it.
                if !self.has_cr {
                    loop {
                        let value_start = data_value_start(&frame, read_idx, data_end);

                        match memchr(LF, &frame[value_start..data_end]) {
                            Some(offset) => {
                                let pos = value_start + offset;

                                frame[write_idx] = LF;
                                write_idx += 1;
                                frame.copy_within(value_start..pos, write_idx);
                                write_idx += offset;

                                read_idx = pos + 1;
                            }

                            None => {
                                frame[write_idx] = LF;
                                write_idx += 1;
                                frame.copy_within(value_start..data_end, write_idx);
                                write_idx += data_end - value_start;

                                break;
                            }
                        }
                    }
                } else {
                    loop {
                        let value_start = data_value_start(&frame, read_idx, data_end);

                        match memchr2(CR, LF, &frame[value_start..data_end]) {
                            Some(offset) => {
                                let eol_idx = value_start + offset;

                                frame[write_idx] = LF;
                                write_idx += 1;
                                frame.copy_within(value_start..eol_idx, write_idx);
                                write_idx += offset;

                                read_idx = eol_idx + 1;

                                if frame[eol_idx] == CR && frame.get(read_idx) == Some(&LF) {
                                    read_idx += 1;
                                }
                            }

                            None => {
                                frame[write_idx] = LF;
                                write_idx += 1;
                                frame.copy_within(value_start..data_end, write_idx);
                                write_idx += data_end - value_start;

                                break;
                            }
                        }
                    }
                }

                frame.truncate(write_idx);
                frame.advance(first_range.start);

                ByteString::try_from(frame)
            } else {
                let mut data = Vec::with_capacity(data_len);
                data.extend_from_slice(&src[first_range]);

                let mut read_idx = next_start;

                if !self.has_cr {
                    loop {
                        let value_start = data_value_start(src, read_idx, data_end);

                        match memchr(LF, &src[value_start..data_end]) {
                            Some(offset) => {
                                let pos = value_start + offset;

                                data.push(LF);
                                data.extend_from_slice(&src[value_start..pos]);

                                read_idx = pos + 1;
                            }

                            None => {
                                data.push(LF);
                                data.extend_from_slice(&src[value_start..data_end]);

                                break;
                            }
                        }
                    }
                } else {
                    loop {
                        let value_start = data_value_start(src, read_idx, data_end);

                        match memchr2(CR, LF, &src[value_start..data_end]) {
                            Some(offset) => {
                                let pos = value_start + offset;

                                data.push(LF);
                                data.extend_from_slice(&src[value_start..pos]);

                                read_idx = pos + 1;

                                if src[pos] == CR && src.get(read_idx) == Some(&LF) {
                                    read_idx += 1;
                                }
                            }

                            None => {
                                data.push(LF);
                                data.extend_from_slice(&src[value_start..data_end]);

                                break;
                            }
                        }
                    }
                }

                src.advance(consumed);

                ByteString::try_from(data)
            }
        };

        Ok(Some(Event::Message(Message {
            data: data?,
            event,
            retry: self.retry,
            id,
        })))
    }

    /// Extracts `(event, id)` values from `src`.
    ///
    /// Validates UTF-8 in the order the fields appeared in the frame so error precedence matches
    /// `ParsedFrame`.
    fn extract_headers(
        &self,
        src: &[u8],
    ) -> Result<(Option<ByteString>, Option<ByteString>), str::Utf8Error> {
        match (self.event, self.id) {
            (None, None) => Ok((None, None)),

            (Some(event), None) => Ok((Some(ByteString::try_from(&src[event.as_range()])?), None)),

            (None, Some(id)) => Ok((None, parse_id_value(&src[id.as_range()])?)),

            (Some(event), Some(id)) if id.start < event.start => {
                let id = parse_id_value(&src[id.as_range()])?;
                let event = ByteString::try_from(&src[event.as_range()])?;

                Ok((Some(event), id))
            }

            (Some(event), Some(id)) => {
                let event = ByteString::try_from(&src[event.as_range()])?;
                let id = parse_id_value(&src[id.as_range()])?;

                Ok((Some(event), id))
            }
        }
    }

    /// Resets the recorded frame state.
    pub(crate) fn clear(&mut self) {
        self.event = None;
        self.id = None;
        self.retry = None;
        self.data_end = 0;
        self.data_len = 0;
        self.data_count = 0;
        self.next_line = 0;
        self.data_closed = false;
        self.has_cr = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_line_rejects_unsupported_layouts() {
        for input in [
            b": comment\n".as_slice(),
            b"unknown: value\n",
            b"event: a\nevent: b\n",
            b"id: 1\nid: 2\n",
            b"data: a\nevent: foo\ndata: b\n",
            b"data: a\nid: 1\ndata: b\n",
            b"data: a\nretry: 100\ndata: b\n",
        ] {
            let mut frame = MessageFrame::default();
            let mut accepted_all = true;

            for (pos, &byte) in input.iter().enumerate() {
                if byte == LF && !frame.record_line(input, pos) {
                    accepted_all = false;
                    break;
                }
            }

            assert!(
                !accepted_all,
                "expected MessageFrame::record_line to reject {:?}",
                str::from_utf8(input),
            );
        }
    }
}
