use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll, ready},
};

use futures_util::Stream;

pin_project_lite::pin_project! {
    /// Converts stream with item `T` into `Result<T, Infallible>`.
    pub struct InfallibleStream<S> {
        #[pin]
        stream: S,
    }
}

impl<S> InfallibleStream<S> {
    /// Constructs new `InfallibleStream` stream.
    pub fn new(stream: S) -> Self {
        Self { stream }
    }
}

impl<S: Stream> Stream for InfallibleStream<S> {
    type Item = Result<S::Item, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(ready!(self.project().stream.poll_next(cx)).map(Ok))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
