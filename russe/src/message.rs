use std::time::Duration;

use bytestring::ByteString;

/// An SSE data message.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// Message data.
    pub data: ByteString,

    /// Name of event.
    pub event: Option<ByteString>,

    /// Recommended retry delay in milliseconds.
    pub retry: Option<Duration>,

    /// Event identifier.
    ///
    /// Used in Last-Event-ID header.
    ///
    /// See <https://github.com/whatwg/html/issues/7363>.
    pub id: Option<ByteString>,
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Message {
        pub(crate) fn data(data: impl Into<ByteString>) -> Self {
            Self {
                data: data.into(),
                ..Default::default()
            }
        }
    }

    // simplifies some tests
    #[allow(clippy::derivable_impls)]
    impl Default for Message {
        fn default() -> Self {
            Self {
                retry: None,
                event: None,
                data: ByteString::new(),
                id: None,
            }
        }
    }
}
