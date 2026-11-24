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

/// Parses an SSE `id` line, ignoring valid UTF-8 values that contain NUL.
pub(crate) fn parse_id(line: &[u8]) -> Result<Option<ByteString>, str::Utf8Error> {
    let id = field_value(line, b"id")
        .map(ByteString::try_from)
        .transpose()?;

    Ok(id.filter(|id| !id.as_bytes().contains(&0)))
}

/// Parses an SSE `retry` field value in milliseconds into a `Duration`.
pub(crate) fn parse_retry(value: &[u8]) -> Option<Duration> {
    // SSE accepts only nonempty ASCII digit sequences.
    if value.is_empty() {
        return None;
    }

    let mut millis = 0u64;

    for (index, &byte) in value.iter().enumerate() {
        let digit = byte.wrapping_sub(b'0');

        if digit > 9 {
            return None;
        }

        let Some(next) = millis
            .checked_mul(10)
            .and_then(|millis| millis.checked_add(u64::from(digit)))
        else {
            return parse_retry_u128(u128::from(millis), &value[index..]);
        };

        millis = next;
    }

    Some(Duration::from_millis(millis))
}

/// Continues parsing a `retry` value after the `u64` millisecond accumulator overflows.
#[cold]
#[inline(never)]
fn parse_retry_u128(mut millis: u128, rest: &[u8]) -> Option<Duration> {
    for &byte in rest {
        let digit = byte.wrapping_sub(b'0');

        if digit > 9 {
            return None;
        }

        millis = millis.checked_mul(10)?.checked_add(u128::from(digit))?;
    }

    let seconds = u64::try_from(millis / 1_000).ok()?;
    let nanos = (millis % 1_000) as u32 * 1_000_000;

    Some(Duration::new(seconds, nanos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_retry_accepts_valid_u64_values() {
        assert_eq!(parse_retry(b"0"), Some(Duration::from_millis(0)));
        assert_eq!(parse_retry(b"42"), Some(Duration::from_millis(42)));
        assert_eq!(parse_retry(b"00042"), Some(Duration::from_millis(42)));
        assert_eq!(parse_retry(b"1500"), Some(Duration::from_millis(1500)));
        assert_eq!(
            parse_retry(b"18446744073709551615"),
            Some(Duration::from_millis(u64::MAX)),
        );
    }

    #[test]
    fn parse_retry_rejects_empty_and_non_digits() {
        for value in [
            b"".as_slice(),
            b"invalid",
            b"-1",
            b"+1",
            b"1.5",
            b" 1",
            b"1 ",
            b"12a3",
            b"0/",
            b"0:",
            b"\xff",
            "١".as_bytes(),
        ] {
            assert_eq!(parse_retry(value), None, "input: {value:?}");
        }
    }

    #[test]
    fn parse_retry_handles_u64_overflow_and_trailing_non_digits() {
        assert_eq!(
            parse_retry(b"18446744073709551616"),
            Some(Duration::new(18_446_744_073_709_551, 616_000_000)),
        );
        assert_eq!(
            parse_retry(b"18446744073709552000"),
            Some(Duration::from_secs(18_446_744_073_709_552)),
        );
        assert_eq!(
            parse_retry(b"18446744073709551615999"),
            Some(Duration::new(u64::MAX, 999_000_000)),
        );

        for value in [
            b"18446744073709551616000".as_slice(),
            b"340282366920938463463374607431768211456",
            b"18446744073709551615x",
            b"18446744073709551616x",
            b"18446744073709551616 ",
            b"18446744073709551615999a",
            b"340282366920938463463374607431768211456x",
        ] {
            assert_eq!(parse_retry(value), None, "input: {value:?}");
        }
    }
}
