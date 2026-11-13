#![allow(missing_docs)]

use std::{io, pin::pin, time::Duration};

use bytes::{Bytes, BytesMut};
use bytestring::ByteString;
use divan::{AllocProfiler, Bencher, black_box};
use futures_test::stream::StreamTestExt as _;
use futures_util::{StreamExt as _, stream};
use russe::{Decoder, Event, Message};
use tokio_util::{
    codec::{Decoder as _, FramedRead},
    io::StreamReader,
};

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

const MIXED_INPUT: &str = indoc::indoc! {"
    retry: 444

    : begin by specifying retry duration

    data: msg1 simple

    data: msg2
    data: with more on a newline

    data:msg3 without optional leading space

    data: msg4 with an ID
    id: 42

    retry: 999
    data: msg5 specifies new retry
    id: 43a

    event: msg
    data: msg6 is named

"};

fn data_message(data: impl Into<ByteString>) -> Message {
    Message {
        data: data.into(),
        event: None,
        retry: None,
        id: None,
    }
}

fn mixed_events() -> Vec<Event> {
    vec![
        Event::Retry(Duration::from_millis(444)),
        Event::Comment("begin by specifying retry duration".into()),
        Event::Message(data_message("msg1 simple")),
        Event::Message(data_message("msg2\nwith more on a newline")),
        Event::Message(data_message("msg3 without optional leading space")),
        Event::Message(Message {
            id: Some("42".into()),
            ..data_message("msg4 with an ID")
        }),
        Event::Message(Message {
            id: Some("43a".into()),
            retry: Some(Duration::from_millis(999)),
            ..data_message("msg5 specifies new retry")
        }),
        Event::Message(Message {
            event: Some("msg".into()),
            ..data_message("msg6 is named")
        }),
    ]
}

fn decode_chunks(
    decoder: &mut Decoder,
    buffer: &mut BytesMut,
    input: &[u8],
    chunk_size: usize,
    mut consume: impl FnMut(Event),
) -> usize {
    let mut count = 0;

    for chunk in input.chunks(chunk_size) {
        buffer.extend_from_slice(chunk);

        while let Some(event) = decoder.decode(buffer).unwrap() {
            consume(event);
            count += 1;
        }
    }

    count
}

fn bench_decode(b: Bencher<'_, '_>, input: &str, expected: &[Event], chunk_size: usize) {
    let mut actual = Vec::new();
    let mut buffer = BytesMut::with_capacity(input.len());

    decode_chunks(
        &mut Decoder::default(),
        &mut buffer,
        input.as_bytes(),
        chunk_size,
        |event| actual.push(event),
    );

    assert_eq!(actual, expected);
    assert!(buffer.is_empty());

    // Exclude parser construction and the initial input buffer allocation.
    b.with_inputs(|| (Decoder::default(), BytesMut::with_capacity(input.len())))
        .bench_refs(|(decoder, buffer)| {
            decode_chunks(
                decoder,
                buffer,
                black_box(input.as_bytes()),
                chunk_size,
                |event| {
                    black_box(event);
                },
            )
        });
}

#[divan::bench]
fn decode_startup() -> Decoder {
    Decoder::default()
}

#[divan::bench(args = [32, 1000, 1024, 60000, 65536])]
fn decode_message(b: Bencher<'_, '_>, size: usize) {
    let data = "x".repeat(size);
    let input = format!("data: {data}\n\n");

    bench_decode(
        b,
        &input,
        &[Event::Message(data_message(data))],
        input.len(),
    );
}

#[divan::bench(args = [
    (32, 1),
    (32, 7),
    (1000, 13),
    (1024, 1),
    (1024, 7),
    (60000, 1500),
    (65536, 7),
    (65536, 1024),
])]
fn decode_fragmented_message(b: Bencher<'_, '_>, (size, chunk_size): (usize, usize)) {
    let data = "x".repeat(size);
    let input = format!("data: {data}\n\n");

    bench_decode(b, &input, &[Event::Message(data_message(data))], chunk_size);
}

#[divan::bench(args = ["lf", "crlf", "cr"])]
fn decode_line_endings(b: Bencher<'_, '_>, name: &str) {
    let ending = match name {
        "lf" => "\n",
        "crlf" => "\r\n",
        "cr" => "\r",
        _ => unreachable!(),
    };
    let input = ": heartbeat\nid: 42\nevent: update\nretry: 1500\ndata: café\ndata: 世界\n\n"
        .replace('\n', ending);
    let expected = [
        Event::Comment("heartbeat".into()),
        Event::Message(Message {
            data: "café\n世界".into(),
            id: Some("42".into()),
            event: Some("update".into()),
            retry: Some(Duration::from_millis(1500)),
        }),
    ];

    bench_decode(b, &input, &expected, 7);
}

#[divan::bench(args = [1, 16, 128])]
fn decode_message_batch(b: Bencher<'_, '_>, count: usize) {
    let input = "data: payload\n\n".repeat(count);
    let expected = vec![Event::Message(data_message("payload")); count];

    bench_decode(b, &input, &expected, input.len());
}

#[divan::bench]
fn decode_ignored_frames(b: Bencher<'_, '_>) {
    let input = format!(
        "{}data: payload\n\n",
        "unknown: ignored\nretry: invalid\n\n\n\n".repeat(64),
    );

    bench_decode(
        b,
        &input,
        &[Event::Message(data_message("payload"))],
        input.len(),
    );
}

async fn decode_stream(input: &str, mut consume: impl FnMut(Event)) -> usize {
    let body_stream = stream::iter(input.as_bytes().chunks(7))
        .map(|chunk| Ok::<_, io::Error>(Bytes::copy_from_slice(chunk)))
        .interleave_pending();
    let body_reader = StreamReader::new(body_stream);
    let event_stream = FramedRead::new(body_reader, Decoder::default());
    let mut event_stream = pin!(event_stream);
    let mut count = 0;

    while let Some(event) = event_stream.next().await {
        consume(event.unwrap());
        count += 1;
    }

    count
}

#[divan::bench]
fn decode_sse_events(b: Bencher<'_, '_>) {
    let rt = tokio::runtime::Handle::current();
    let mut actual = Vec::new();

    rt.block_on(decode_stream(MIXED_INPUT, |event| actual.push(event)));
    assert_eq!(actual, mixed_events());

    b.bench(|| {
        let count = rt.block_on(decode_stream(black_box(MIXED_INPUT), |event| {
            black_box(event);
        }));

        assert_eq!(count, 8);

        count
    });
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();

    divan::main();
}
