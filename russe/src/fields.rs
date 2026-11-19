use std::{str, time::Duration};

use bytestring::ByteString;

use crate::{Event, Message};

pub(crate) fn data_event(data: ByteString) -> Event {
    Event::Message(Message {
        data,
        event: None,
        retry: None,
        id: None,
    })
}

pub(crate) fn strip_space(value: &[u8]) -> &[u8] {
    value.strip_prefix(b" ").unwrap_or(value)
}

pub(crate) fn field_value<'a>(line: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
    match line.strip_prefix(field)? {
        [] => Some(&[]),
        [b':', value @ ..] => Some(strip_space(value)),
        _ => None,
    }
}

pub(crate) fn parse_retry(value: &[u8]) -> Option<Duration> {
    // SSE accepts only nonempty ASCII digit sequences.
    if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
        return None;
    }

    // Duration can hold more than u64::MAX milliseconds.
    let millis: u128 = str::from_utf8(value).ok()?.parse().ok()?;
    let seconds = u64::try_from(millis / 1_000).ok()?;
    let nanos = (millis % 1_000) as u32 * 1_000_000;

    Some(Duration::new(seconds, nanos))
}
