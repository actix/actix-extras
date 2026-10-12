use std::{fmt::Write, time::Duration};

use bytes::BytesMut;
use bytestring::ByteString;

use crate::{Error, Message};

/// An SSE event.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Message event.
    Message(Message),

    /// Comment event.
    Comment(ByteString),

    /// Retry recommendation event.
    Retry(Duration),
}

/// An SSE event that borrows its field text.
///
/// Use this type to encode text without allocating or cloning [`ByteString`] inputs.
///
/// # Examples
///
/// ```
/// use bytes::BytesMut;
/// use russe::{Encoder, EventRef};
///
/// let text = String::from("first\nsecond");
/// let event = EventRef::Message {
///     data: &text,
///     event: Some("update"),
///     retry: None,
///     id: Some("42"),
/// };
/// let mut output = BytesMut::new();
///
/// Encoder::default().encode_borrowed(event, &mut output)?;
/// assert_eq!(
///     output,
///     "id: 42\nevent: update\ndata: first\ndata: second\n\n"
/// );
/// # Ok::<_, russe::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventRef<'a> {
    /// Message event.
    Message {
        /// Message data.
        data: &'a str,

        /// Name of event.
        event: Option<&'a str>,

        /// Recommended retry delay.
        retry: Option<Duration>,

        /// Event identifier.
        id: Option<&'a str>,
    },

    /// Comment event.
    Comment(&'a str),

    /// Retry recommendation event.
    Retry(Duration),
}

impl<'a> From<&'a Event> for EventRef<'a> {
    fn from(event: &'a Event) -> Self {
        match event {
            Event::Message(message) => Self::Message {
                data: &message.data,
                event: message.event.as_deref(),
                retry: message.retry,
                id: message.id.as_deref(),
            },
            Event::Comment(comment) => Self::Comment(comment),
            Event::Retry(retry) => Self::Retry(*retry),
        }
    }
}

impl Event {
    /// Encodes the event into a UTF-8 string, including the final blank line.
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be encoded. See [`crate::Encoder`] for field requirements.
    ///
    /// # Examples
    ///
    /// ```
    /// use russe::Event;
    ///
    /// let encoded = Event::Comment("keep-alive".into()).into_bytestring()?;
    /// assert_eq!(encoded, ": keep-alive\n\n");
    /// # Ok::<_, russe::Error>(())
    /// ```
    pub fn into_bytestring(self) -> crate::Result<ByteString> {
        let mut dst = String::new();

        self.encode_into(&mut dst)?;

        Ok(dst.into())
    }

    pub(crate) fn encode_into(self, dst: &mut impl EncodeBuffer) -> crate::Result<()> {
        match self {
            Event::Message(message) => {
                if let Some(id) = message.id.as_deref()
                    && let Some(offset) = memchr::memchr3(b'\0', b'\r', b'\n', id.as_bytes())
                {
                    return Err(Error::invalid_field_value("id", id, offset));
                }

                if let Some(event) = message.event.as_deref()
                    && let Some(offset) = memchr::memchr2(b'\r', b'\n', event.as_bytes())
                {
                    return Err(Error::invalid_field_value("event", event, offset));
                }

                // Start with the known payload and field lengths.
                let capacity = message.data.len()
                    + 8
                    + message.id.as_ref().map_or(0, |id| id.len() + 5)
                    + message.event.as_ref().map_or(0, |event| event.len() + 8)
                    + message.retry.map_or(0, |_| RETRY_CAPACITY);

                // For data larger than 1 KiB, sample 128 bytes for short lines.
                // Round the reservation so added prefixes use power-of-two growth.
                let capacity = if message.data.len() > 1024
                    && dst.spare_capacity() < capacity
                    && memchr::memchr2(b'\r', b'\n', &message.data.as_bytes()[..128]).is_some()
                {
                    capacity.checked_next_power_of_two().unwrap_or(capacity)
                } else {
                    capacity
                };

                dst.reserve(capacity);

                if let Some(id) = message.id {
                    let _ = dst.write_str("id: ");
                    let _ = dst.write_str(&id);
                    let _ = dst.write_char('\n');
                }

                if let Some(event) = message.event {
                    let _ = dst.write_str("event: ");
                    let _ = dst.write_str(&event);
                    let _ = dst.write_char('\n');
                }

                if let Some(retry) = message.retry {
                    encode_retry(dst, retry);
                }

                encode_lines(dst, "data: ", &message.data);
                let _ = dst.write_char('\n');
            }

            Event::Comment(comment) => {
                dst.reserve(comment.len() + 4);

                encode_lines(dst, ": ", &comment);
                let _ = dst.write_char('\n');
            }

            Event::Retry(retry) => {
                dst.reserve(RETRY_CAPACITY);

                encode_retry(dst, retry);
                let _ = dst.write_char('\n');
            }
        }

        Ok(())
    }
}

impl EventRef<'_> {
    /// Encodes the event into a UTF-8 string, including the final blank line.
    ///
    /// # Errors
    ///
    /// Returns an error if a field is invalid. See [`crate::Encoder`] for field requirements.
    pub fn into_bytestring(self) -> crate::Result<ByteString> {
        let mut dst = String::new();

        self.encode_into(&mut dst)?;

        Ok(dst.into())
    }

    pub(crate) fn encode_into(self, dst: &mut impl EncodeBuffer) -> crate::Result<()> {
        match self {
            Self::Message {
                data,
                event,
                retry,
                id,
            } => return encode_borrowed_message(dst, data, event, retry, id),
            Self::Comment(comment) => encode_comment(dst, comment),
            Self::Retry(retry) => encode_retry_event(dst, retry),
        }

        Ok(())
    }
}

fn encode_borrowed_message(
    dst: &mut impl EncodeBuffer,
    data: &str,
    event: Option<&str>,
    retry: Option<Duration>,
    id: Option<&str>,
) -> crate::Result<()> {
    if let Some(id) = id
        && let Some(offset) = memchr::memchr3(b'\0', b'\r', b'\n', id.as_bytes())
    {
        return Err(Error::invalid_field_value("id", id, offset));
    }

    if let Some(event) = event
        && let Some(offset) = memchr::memchr2(b'\r', b'\n', event.as_bytes())
    {
        return Err(Error::invalid_field_value("event", event, offset));
    }

    // Start with the known payload and field lengths.
    let capacity = data.len()
        + 8
        + id.map_or(0, |id| id.len() + 5)
        + event.map_or(0, |event| event.len() + 8)
        + retry.map_or(0, |_| RETRY_CAPACITY);

    // For data larger than 1 KiB, sample 128 bytes for short lines.
    // Round the reservation so added prefixes use power-of-two growth.
    let capacity = if data.len() > 1024
        && dst.spare_capacity() < capacity
        && memchr::memchr2(b'\r', b'\n', &data.as_bytes()[..128]).is_some()
    {
        capacity.checked_next_power_of_two().unwrap_or(capacity)
    } else {
        capacity
    };

    dst.reserve(capacity);

    if let Some(id) = id {
        let _ = dst.write_str("id: ");
        let _ = dst.write_str(id);
        let _ = dst.write_char('\n');
    }

    if let Some(event) = event {
        let _ = dst.write_str("event: ");
        let _ = dst.write_str(event);
        let _ = dst.write_char('\n');
    }

    if let Some(retry) = retry {
        encode_retry(dst, retry);
    }

    encode_lines(dst, "data: ", data);
    let _ = dst.write_char('\n');

    Ok(())
}

fn encode_comment(dst: &mut impl EncodeBuffer, comment: &str) {
    dst.reserve(comment.len() + 4);

    encode_lines(dst, ": ", comment);
    let _ = dst.write_char('\n');
}

fn encode_retry_event(dst: &mut impl EncodeBuffer, retry: Duration) {
    dst.reserve(RETRY_CAPACITY);

    encode_retry(dst, retry);
    let _ = dst.write_char('\n');
}

// Start with 16 bytes for common retry values; larger values can grow the buffer.
const RETRY_CAPACITY: usize = 16;

pub(crate) trait EncodeBuffer: Write {
    fn reserve(&mut self, additional: usize);

    fn spare_capacity(&self) -> usize;

    fn write_lines(&mut self, prefix: &str, text: &str);
}

impl EncodeBuffer for String {
    fn reserve(&mut self, additional: usize) {
        String::reserve(self, additional);
    }

    fn spare_capacity(&self) -> usize {
        self.capacity() - self.len()
    }

    fn write_lines(&mut self, prefix: &str, text: &str) {
        encode_string_lines(self, prefix, text);
    }
}

impl EncodeBuffer for BytesMut {
    fn reserve(&mut self, additional: usize) {
        BytesMut::reserve(self, additional);
    }

    fn spare_capacity(&self) -> usize {
        self.capacity() - self.len()
    }

    fn write_lines(&mut self, prefix: &str, text: &str) {
        encode_byte_lines(self, prefix.as_bytes(), text.as_bytes());
    }
}

fn encode_retry(dst: &mut impl Write, retry: Duration) {
    let mut buffer = itoa::Buffer::new();

    let _ = dst.write_str("retry: ");
    let _ = dst.write_str(buffer.format(retry.as_millis()));
    let _ = dst.write_char('\n');
}

fn encode_lines(dst: &mut impl EncodeBuffer, prefix: &str, text: &str) {
    dst.write_lines(prefix, text);
}

/// Prefixes each line, preserves empty lines, and converts CRLF and CR to LF.
fn encode_string_lines(dst: &mut String, prefix: &str, mut text: &str) {
    loop {
        dst.push_str(prefix);

        let Some(idx) = memchr::memchr2(b'\r', b'\n', text.as_bytes()) else {
            dst.push_str(text);
            dst.push('\n');

            break;
        };

        let line_ending = text.as_bytes()[idx];

        if line_ending == b'\n' {
            // Copy the line and its LF in one write.
            dst.push_str(&text[..=idx]);
        } else {
            dst.push_str(&text[..idx]);
            dst.push('\n');
        }

        text = &text[idx + 1..];

        if line_ending == b'\r' {
            text = text.strip_prefix('\n').unwrap_or(text);
        }
    }
}

/// Prefixes each line, preserves empty lines, and converts CRLF and CR to LF.
fn encode_byte_lines(dst: &mut BytesMut, prefix: &[u8], bytes: &[u8]) {
    let mut start = 0;

    for idx in memchr::memchr2_iter(b'\r', b'\n', bytes) {
        if idx < start {
            continue;
        }

        dst.extend_from_slice(prefix);

        let line_ending = bytes[idx];

        if line_ending == b'\n' {
            // Copy the line and its LF in one write.
            dst.extend_from_slice(&bytes[start..=idx]);
        } else {
            dst.extend_from_slice(&bytes[start..idx]);
            dst.extend_from_slice(b"\n");
        }

        start = idx + 1;

        if line_ending == b'\r' && bytes.get(start) == Some(&b'\n') {
            start += 1;
        }
    }

    dst.extend_from_slice(prefix);
    dst.extend_from_slice(&bytes[start..]);
    dst.extend_from_slice(b"\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_events_into_bytestring() {
        let cases = [
            (
                Event::Message(Message {
                    data: "café\r\n世界".into(),
                    event: Some("update".into()),
                    id: Some("42".into()),
                    retry: None,
                }),
                "id: 42\nevent: update\ndata: café\ndata: 世界\n\n",
            ),
            (Event::Comment("keep-alive".into()), ": keep-alive\n\n"),
            (Event::Retry(Duration::from_millis(1500)), "retry: 1500\n\n"),
        ];

        for (event, expected) in cases {
            assert_eq!(event.into_bytestring().unwrap(), expected);
        }
    }

    #[test]
    fn preserves_empty_lines_and_utf8_with_each_line_ending() {
        for (text, expected) in [
            ("", "data: \n\n"),
            ("\r\n", "data: \ndata: \n\n"),
            ("\r\r\n\n", "data: \ndata: \ndata: \ndata: \n\n"),
            (
                "é\r\n世界\r🦀\n",
                "data: é\ndata: 世界\ndata: 🦀\ndata: \n\n",
            ),
            ("é\0世界", "data: é\0世界\n\n"),
        ] {
            let event = Event::Message(Message::data(text));

            assert_eq!(
                event.into_bytestring().unwrap(),
                expected,
                "input: {text:?}"
            );

            let event = Event::Comment(text.into());

            assert_eq!(
                event.into_bytestring().unwrap(),
                expected.replace("data: ", ": "),
                "input: {text:?}",
            );
        }
    }

    #[test]
    fn encodes_maximum_retry_in_messages_and_retry_events() {
        for event in [
            Event::Retry(Duration::MAX),
            Event::Message(Message {
                retry: Some(Duration::MAX),
                ..Message::data("")
            }),
        ] {
            let expected = match event {
                Event::Retry(_) => "retry: 18446744073709551615999\n\n",
                _ => "retry: 18446744073709551615999\ndata: \n\n",
            };

            assert_eq!(event.into_bytestring().unwrap(), expected);
        }
    }

    #[test]
    fn encodes_retries_across_the_initial_capacity() {
        for millis in [9_999_999, 10_000_000] {
            let retry = Duration::from_millis(millis);

            assert_eq!(
                Event::Retry(retry).into_bytestring().unwrap(),
                format!("retry: {millis}\n\n"),
            );

            let event = Event::Message(Message {
                retry: Some(retry),
                ..Message::data("")
            });

            assert_eq!(
                event.into_bytestring().unwrap(),
                format!("retry: {millis}\ndata: \n\n"),
            );
        }
    }

    #[test]
    fn encodes_multiline_data_with_uneven_line_lengths() {
        let short_lines = "\n".repeat(128);
        let long_line = "é世界".repeat(256);
        let data = format!("{short_lines}{long_line}\r\n");
        let expected = format!("{}data: {long_line}\ndata: \n\n", "data: \n".repeat(128),);
        let event = Event::Message(Message::data(data));

        assert_eq!(event.clone().into_bytestring().unwrap(), expected);

        let mut dst = BytesMut::from("previous event\n\n");

        event.encode_into(&mut dst).unwrap();

        assert_eq!(
            dst.as_ref(),
            format!("previous event\n\n{expected}").as_bytes()
        );
    }

    #[test]
    fn rejects_invalid_message_fields() {
        for (id, event) in [(Some("bad\0id"), None), (None, Some("bad\nevent"))] {
            let message = Message {
                data: "hello".into(),
                event: event.map(Into::into),
                id: id.map(Into::into),
                retry: None,
            };

            assert!(matches!(
                Event::Message(message).into_bytestring(),
                Err(crate::Error::InvalidFieldValue { .. })
            ));
        }
    }

    #[test]
    fn invalid_field_errors_report_details() {
        for (id, event, expected) in [
            (Some("bad\0id"), None, "Invalid SSE id: '\\0' at byte 3"),
            (
                None,
                Some("bad\nevent"),
                "Invalid SSE event: '\\n' at byte 3",
            ),
            (Some("é\rid"), None, "Invalid SSE id: '\\r' at byte 2"),
            (Some("é\r\0id"), None, "Invalid SSE id: '\\r' at byte 2"),
            (Some("é\0\rid"), None, "Invalid SSE id: '\\0' at byte 2"),
            (
                Some("bad\r\nid"),
                Some("bad\nevent"),
                "Invalid SSE id: '\\r' at byte 3",
            ),
            (
                None,
                Some("\"bad\\name\n"),
                "Invalid SSE event: '\\n' at byte 9",
            ),
        ] {
            let message = Message {
                data: "hello".into(),
                event: event.map(Into::into),
                id: id.map(Into::into),
                retry: None,
            };

            let err = Event::Message(message).into_bytestring().unwrap_err();

            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn invalid_field_errors_report_byte_offsets() {
        for (id, expected_offset) in [
            (format!("{}\n{}", "a".repeat(100), "z".repeat(100)), 100),
            (format!("{}\n{}", "é".repeat(100), "世".repeat(100)), 200),
            (
                format!("{}\n{}", "\\".repeat(100), "\u{1}".repeat(100)),
                100,
            ),
            (format!("\n{}", "世".repeat(100)), 0),
            (format!("{}\n", "é".repeat(100)), 200),
        ] {
            let message = Message {
                data: "hello".into(),
                event: None,
                id: Some(id.into()),
                retry: None,
            };

            let err = Event::Message(message).into_bytestring().unwrap_err();
            let Error::InvalidFieldValue {
                field,
                character,
                offset,
            } = &err
            else {
                panic!("Unexpected error: {err:?}");
            };

            assert_eq!((*field, *character, *offset), ("id", '\n', expected_offset));
        }
    }
}
