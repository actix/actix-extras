#![allow(missing_docs)]

use bytes::BytesMut;
use bytestring::ByteString;
use divan::{AllocProfiler, Bencher, black_box};
use russe::{Encoder, Event, EventRef, Message};
use tokio_util::codec::Encoder as _;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

#[divan::bench(args = [32, 1024, 65536])]
fn encode_codec_constructed_message(b: Bencher<'_, '_>, size: usize) {
    let data = ByteString::from("x".repeat(size));
    let id = ByteString::from(String::from("42"));
    let event = ByteString::from(String::from("update"));
    let expected = format!("id: 42\nevent: update\ndata: {data}\n\n");
    let mut output = BytesMut::new();

    Encoder::default()
        .encode(
            Event::Message(Message {
                data: data.clone(),
                event: Some(event.clone()),
                retry: None,
                id: Some(id.clone()),
            }),
            &mut output,
        )
        .unwrap();
    assert_eq!(output.as_ref(), expected.as_bytes());

    b.bench_local(|| {
        let message = Message {
            data: black_box(&data).clone(),
            event: Some(black_box(&event).clone()),
            retry: None,
            id: Some(black_box(&id).clone()),
        };
        let mut output = BytesMut::new();

        Encoder::default()
            .encode(Event::Message(message), &mut output)
            .unwrap();

        output
    });
}

#[divan::bench(args = [32, 1024, 65536])]
fn encode_codec_borrowed_message(b: Bencher<'_, '_>, size: usize) {
    let data = ByteString::from("x".repeat(size));
    let id = ByteString::from(String::from("42"));
    let event = ByteString::from(String::from("update"));
    let expected = format!("id: 42\nevent: update\ndata: {data}\n\n");
    let mut output = BytesMut::new();

    Encoder::default()
        .encode_borrowed(
            EventRef::Message {
                data: &data,
                event: Some(&event),
                retry: None,
                id: Some(&id),
            },
            &mut output,
        )
        .unwrap();
    assert_eq!(output.as_ref(), expected.as_bytes());

    b.bench_local(|| {
        let event = EventRef::Message {
            data: black_box(&data),
            event: Some(black_box(&event)),
            retry: None,
            id: Some(black_box(&id)),
        };
        let mut output = BytesMut::new();

        Encoder::default()
            .encode_borrowed(event, &mut output)
            .unwrap();

        output
    });
}

#[divan::bench(args = [32, 1000, 1024, 60000, 65536])]
fn encode_codec_borrowed_message_reused_buffer(b: Bencher<'_, '_>, size: usize) {
    let data = "x".repeat(size);
    let expected = format!("data: {data}\n\n");
    let event = EventRef::Message {
        data: &data,
        event: None,
        retry: None,
        id: None,
    };
    let mut output = BytesMut::with_capacity(expected.len());
    let mut encoder = Encoder::default();

    encoder.encode_borrowed(event, &mut output).unwrap();
    assert_eq!(output.as_ref(), expected.as_bytes());

    b.with_inputs(|| event).bench_local_values(|event| {
        output.clear();
        encoder.encode_borrowed(event, &mut output).unwrap();
        black_box(output.as_ref());

        output.len()
    });
}

fn main() {
    divan::main();
}
