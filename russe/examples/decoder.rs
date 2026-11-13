//! Demonstrates usage of the codec to parse a response from a response stream.

use std::{io, pin::pin, time::Duration};

use bytes::Bytes;
use futures_util::{Stream, StreamExt as _};
use russe::Decoder as SseDecoder;
use tokio_util::{codec::FramedRead, io::StreamReader};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let body_reader = StreamReader::new(chunk_stream());

    let event_stream = FramedRead::new(body_reader, SseDecoder::default());
    let mut event_stream = pin!(event_stream);

    while let Some(Ok(ev)) = event_stream.next().await {
        println!("{ev:?}");
    }
}

fn chunk_stream() -> impl Stream<Item = io::Result<Bytes>> {
    use tokio_test::stream_mock::StreamMockBuilder;

    let input = indoc::indoc! {"
            event: add
            data: foo
            id: 1
            
            : keep-alive
            
            event: remove
            data: bar
            id: 2

            "};

    let mut mock = StreamMockBuilder::new();

    // emulate chunked transfer and small delays between chunks
    for chunk in input.as_bytes().chunks(7) {
        mock = mock.next(Bytes::from(chunk));
        mock = mock.wait(Duration::from_millis(80));
    }

    mock.build().map(Ok)
}
