use std::{str, time::Duration};

use bytestring::ByteString;

use crate::{Event, Message};

/// Builds a data-only message event.
pub(crate) fn data_event(data: ByteString) -> Event {
    Event::Message(Message {
        data,
        event: None,
        retry: None,
        id: None,
    })
}

/// Removes at most one leading ASCII space from an SSE field value.
pub(crate) fn strip_space(value: &[u8]) -> &[u8] {
    value.strip_prefix(b" ").unwrap_or(value)
}

/// Returns the field value slice if `line` has the given `field` name.
///
/// Accepts both bare `field` (empty value) and `field:value`.
pub(crate) fn field_value<'a>(line: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
    match line.strip_prefix(field)? {
        [] => Some(&[]),
        [b':', value @ ..] => Some(strip_space(value)),
        _ => None,
    }
}

/// Parses an SSE `retry` field value in milliseconds into a `Duration`.
pub(crate) fn parse_retry(value: &[u8]) -> Option<Duration> {
    // SSE accepts only nonempty ASCII digit sequences.
    if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
        return None;
    }

    // Duration can hold more than u64::MAX milliseconds.
    let millis = str::from_utf8(value).ok()?.parse::<u128>().ok()?;
    let seconds = u64::try_from(millis / 1_000).ok()?;
    let nanos = (millis % 1_000) as u32 * 1_000_000;

    Some(Duration::new(seconds, nanos))
}
