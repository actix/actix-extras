#![allow(missing_docs)]

use std::time::Duration;

use bytes::BytesMut;
use bytestring::ByteString;
use divan::{AllocProfiler, Bencher, black_box};
use russe::{Encoder, Event, Message};
use tokio_util::codec::Encoder as _;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn data_message(data: impl Into<ByteString>) -> Message {
    Message {
        data: data.into(),
        event: None,
        retry: None,
        id: None,
    }
}

fn bench_event(b: Bencher<'_, '_>, event: Event, expected: &str) {
    assert_eq!(event.clone().into_bytestring().unwrap(), expected);

    b.with_inputs(|| event.clone())
        .bench_values(|event| event.into_bytestring().unwrap());
}

#[divan::bench(args = [32, 1000, 1024, 60000, 65536])]
fn encode_into_bytestring_message(b: Bencher<'_, '_>, size: usize) {
    let data = "x".repeat(size);
    let expected = format!("data: {data}\n\n");

    bench_event(b, Event::Message(data_message(data)), &expected);
}

#[divan::bench]
fn encode_into_bytestring_multiline_message(b: Bencher<'_, '_>) {
    let expected = format!(
        "id: 42\nevent: update\nretry: 1500\n{}\n",
        "data: café\ndata: 世界\ndata: \ndata: next\n".repeat(64) + "data: \n",
    );

    bench_event(
        b,
        Event::Message(Message {
            data: "café\r\n世界\n\nnext\r".repeat(64).into(),
            event: Some("update".into()),
            retry: Some(Duration::from_millis(1500)),
            id: Some("42".into()),
        }),
        &expected,
    );
}

#[divan::bench(args = [6, 32, 1024])]
fn encode_codec_message_fields_reused_buffer(b: Bencher<'_, '_>, size: usize) {
    let field = ByteString::from("x".repeat(size));
    let event = Event::Message(Message {
        data: "payload".into(),
        event: Some(field.clone()),
        retry: None,
        id: Some(field),
    });
    let expected = format!(
        "id: {}\nevent: {}\ndata: payload\n\n",
        "x".repeat(size),
        "x".repeat(size)
    );
    let mut output = BytesMut::with_capacity(expected.len());
    let mut encoder = Encoder::default();

    encoder.encode(event.clone(), &mut output).unwrap();
    assert_eq!(output.as_ref(), expected.as_bytes());

    b.with_inputs(|| event.clone()).bench_local_values(|event| {
        output.clear();
        encoder.encode(event, &mut output).unwrap();
        black_box(output.as_ref());

        output.len()
    });
}

#[divan::bench]
fn encode_into_bytestring_comment(b: Bencher<'_, '_>) {
    bench_event(b, Event::Comment("keep-alive".into()), ": keep-alive\n\n");
}

#[divan::bench]
fn encode_into_bytestring_retry(b: Bencher<'_, '_>) {
    bench_event(
        b,
        Event::Retry(Duration::from_millis(1500)),
        "retry: 1500\n\n",
    );
}

fn bench_codec_message(b: Bencher<'_, '_>, size: usize) {
    let data = "x".repeat(size);
    let expected = format!("data: {data}\n\n");
    let event = Event::Message(data_message(data));
    let mut output = BytesMut::new();

    Encoder::default()
        .encode(event.clone(), &mut output)
        .unwrap();
    assert_eq!(output.as_ref(), expected.as_bytes());

    b.with_inputs(|| event.clone()).bench_values(|event| {
        let mut output = BytesMut::new();

        Encoder::default().encode(event, &mut output).unwrap();

        output
    });
}

#[divan::bench(args = [32, 1000, 1024, 60000, 65536])]
fn encode_codec_message_new_buffer(b: Bencher<'_, '_>, size: usize) {
    bench_codec_message(b, size);
}

#[divan::bench(args = [32, 1024, 65536])]
fn encode_codec_frame_size_new_buffer(b: Bencher<'_, '_>, frame_size: usize) {
    // Include the data field prefix and final blank line in the frame size.
    bench_codec_message(b, frame_size - "data: \n\n".len());
}

#[divan::bench(args = [32, 1000, 1024, 60000, 65536])]
fn encode_codec_message_reused_buffer(b: Bencher<'_, '_>, size: usize) {
    let data = "x".repeat(size);
    let expected = format!("data: {data}\n\n");
    let event = Event::Message(data_message(data));
    let mut output = BytesMut::with_capacity(expected.len());
    let mut encoder = Encoder::default();

    encoder.encode(event.clone(), &mut output).unwrap();
    assert_eq!(output.as_ref(), expected.as_bytes());

    b.with_inputs(|| event.clone()).bench_local_values(|event| {
        output.clear();
        encoder.encode(event, &mut output).unwrap();
        black_box(output.as_ref());

        output.len()
    });
}

#[divan::bench(args = [64, 4096])]
fn encode_codec_multiline_message_reused_buffer(b: Bencher<'_, '_>, lines: usize) {
    let data = "0123456789abcde\n".repeat(lines);
    let expected = "data: 0123456789abcde\n".repeat(lines) + "data: \n\n";
    let event = Event::Message(data_message(data));
    let mut output = BytesMut::with_capacity(expected.len());
    let mut encoder = Encoder::default();

    encoder.encode(event.clone(), &mut output).unwrap();
    assert_eq!(output.as_ref(), expected.as_bytes());

    b.with_inputs(|| event.clone()).bench_local_values(|event| {
        output.clear();
        encoder.encode(event, &mut output).unwrap();
        black_box(output.as_ref());

        output.len()
    });
}

#[divan::bench(args = [0, 1500, u64::MAX])]
fn encode_codec_retry_reused_buffer(b: Bencher<'_, '_>, millis: u64) {
    let event = Event::Retry(Duration::from_millis(millis));
    let expected = format!("retry: {millis}\n\n");
    let mut output = BytesMut::with_capacity(expected.len());
    let mut encoder = Encoder::default();

    encoder.encode(event.clone(), &mut output).unwrap();
    assert_eq!(output.as_ref(), expected.as_bytes());

    b.bench_local(|| {
        output.clear();
        encoder
            .encode(black_box(event.clone()), &mut output)
            .unwrap();
        black_box(output.as_ref());

        output.len()
    });
}

fn main() {
    divan::main();
}
