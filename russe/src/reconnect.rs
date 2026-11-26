use std::time::Duration;

use bytestring::ByteString;
use futures_util::{Stream, StreamExt as _};
use tokio::sync::{mpsc::UnboundedSender, watch};

use crate::Event;

/// Controls what a connection manager does when a response stream ends.
///
/// Each reconnect request counts toward `max_retries`, including failed requests.
/// The initial request is separate. A limit of zero disables reconnection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reconnect {
    /// Close the event channel at the end of the first response.
    #[default]
    Never,

    /// Reconnect after a body or decoding error.
    ///
    /// A clean response stream EoF closes the channel.
    ///
    /// An SSE retry field updates the delay for subsequent attempts.
    ///
    /// The delay follows SSE retry fields as in [`Self::OnEof`].
    OnError {
        /// Initial delay between attempts. SSE retry fields can update this delay.
        delay: Duration,

        /// Maximum number of reconnect requests for this stream.
        max_retries: usize,
    },

    /// Reconnect on clean response stream EoF after this delay.
    /// A body or decoding error is sent to the receiver and closes the channel.
    ///
    /// An SSE retry field updates the delay for subsequent attempts.
    ///
    /// HTTP 204 stops reconnection.
    ///
    /// Failed reconnect requests go to the receiver and are retried.
    OnEof {
        /// Initial delay between attempts. SSE retry fields can update this delay.
        delay: Duration,

        /// Maximum number of reconnect requests for this stream.
        max_retries: usize,
    },

    /// Reconnect after clean EOF or a stream error.
    ///
    /// HTTP 204 stops reconnection.
    ///
    /// Stream and request errors are sent to the receiver for logging.
    Always {
        /// Initial delay between attempts. SSE retry fields can update this delay.
        delay: Duration,

        /// Maximum number of reconnect requests for this stream.
        max_retries: usize,
    },
}

pub(crate) async fn run<S, E>(
    initial_stream: Option<S>,
    policy: Reconnect,
    ids: watch::Receiver<Option<ByteString>>,
    tx: UnboundedSender<Result<Event, E>>,
    reconnect: impl AsyncFn(Option<ByteString>) -> Result<Option<S>, E>,
) where
    S: Stream<Item = Result<Event, E>> + Unpin,
{
    let mut stream = initial_stream;

    let (mut delay, max_retries) = match policy {
        Reconnect::Never => (Duration::ZERO, 0),
        Reconnect::OnError { delay, max_retries }
        | Reconnect::OnEof { delay, max_retries }
        | Reconnect::Always { delay, max_retries } => (delay, max_retries),
    };

    let mut retries = 0..max_retries;

    let read = async {
        while let Some(mut current) = stream.take() {
            let mut failed = false;

            while let Some(event) = current.next().await {
                failed |= event.is_err();

                match &event {
                    Ok(Event::Retry(retry)) => delay = *retry,
                    Ok(Event::Message(msg)) => delay = msg.retry.unwrap_or(delay),
                    _ => {}
                }

                if tx.send(event).is_err() {
                    return;
                }
            }

            if matches!(
                (policy, failed),
                (Reconnect::OnError { .. }, false) | (Reconnect::OnEof { .. }, true)
            ) {
                return;
            }

            for _ in retries.by_ref() {
                tokio::time::sleep(delay).await;

                let id = ids.borrow().clone();

                match reconnect(id).await {
                    Ok(next) => {
                        stream = next;
                        break;
                    }

                    Err(err) => {
                        if tx.send(Err(err)).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    };

    tokio::select! {
        biased;
        _ = tx.closed() => {}
        _ = read => {}
    }
}
