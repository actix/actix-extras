use std::str;

use actix_web::web::BytesMut;

enum State {
    YieldStr,
    YieldQuote,
}

struct Quoted<'a> {
    inner: ::std::iter::Peekable<str::Split<'a, char>>,
    state: State,
}

impl<'a> Quoted<'a> {
    pub fn new(s: &'a str) -> Quoted<'_> {
        Quoted {
            inner: s.split('"').peekable(),
            state: State::YieldStr,
        }
    }
}

impl<'a> Iterator for Quoted<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match self.state {
            State::YieldStr => match self.inner.next() {
                Some(s) => {
                    self.state = State::YieldQuote;
                    Some(s)
                }
                None => None,
            },
            State::YieldQuote => match self.inner.peek() {
                Some(_) => {
                    self.state = State::YieldStr;
                    Some("\\\"")
                }
                None => None,
            },
        }
    }
}

/// Tries to quote the quotes in the passed `value`
pub fn put_quoted(buf: &mut BytesMut, value: &str) {
    for part in Quoted::new(value) {
        buf.extend_from_slice(part.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use std::str;

    use actix_web::web::BytesMut;

    use super::put_quoted;

    #[test]
    fn test_quote_str() {
        let input = "a \"quoted\" string";
        let mut output = BytesMut::new();
        put_quoted(&mut output, input);
        let result = str::from_utf8(&output).unwrap();

        assert_eq!(result, "a \\\"quoted\\\" string");
    }

    #[test]
    fn test_without_quotes() {
        let input = "non-quoted string";
        let mut output = BytesMut::new();
        put_quoted(&mut output, input);
        let result = str::from_utf8(&output).unwrap();

        assert_eq!(result, "non-quoted string");
    }

    #[test]
    fn test_starts_with_quote() {
        let input = "\"first-quoted string";
        let mut output = BytesMut::new();
        put_quoted(&mut output, input);
        let result = str::from_utf8(&output).unwrap();

        assert_eq!(result, "\\\"first-quoted string");
    }

    #[test]
    fn test_ends_with_quote() {
        let input = "last-quoted string\"";
        let mut output = BytesMut::new();
        put_quoted(&mut output, input);
        let result = str::from_utf8(&output).unwrap();

        assert_eq!(result, "last-quoted string\\\"");
    }

    #[test]
    fn test_double_quote() {
        let input = "quote\"\"string";
        let mut output = BytesMut::new();
        put_quoted(&mut output, input);
        let result = str::from_utf8(&output).unwrap();

        assert_eq!(result, "quote\\\"\\\"string");
    }
}
