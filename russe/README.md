# russe

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/russe?label=latest)](https://crates.io/crates/russe)
[![Documentation](https://docs.rs/russe/badge.svg?version=0.3.5)](https://docs.rs/russe/0.3.5)
![Version](https://img.shields.io/badge/rustc-1.89+-ab6000.svg)
![MIT or Apache 2.0 licensed](https://img.shields.io/crates/l/russe.svg)
<br />
[![Dependency Status](https://deps.rs/crate/russe/0.3.5/status.svg)](https://deps.rs/crate/russe/0.3.5)
[![Download](https://img.shields.io/crates/d/russe.svg)](https://crates.io/crates/russe)
[![Chat on Discord](https://img.shields.io/discord/771444961383153695?label=chat&logo=discord)](https://discord.gg/NWpN5mmg3x)

<!-- prettier-ignore-end -->

<!-- cargo-rdme start -->

High-performance Server-Sent Events (SSE) encoder and decoder.

## Encoding (server-side)

```rust
let event = Event::Message(Message {
    data: "Hello World!\nHow are you, today?".into(),
    event: Some("chat".into()),
    retry: None,
    id: None,
});

let expected = indoc! {"
    event: chat
    data: Hello World!
    data: How are you, today?

"};

assert_eq!(event.into_bytestring()?, expected);
```

## Decoding (client-side)

The client receives the server's encoded `chat` event in the response body.

```rust
let mut body = BytesMut::from(indoc! {"
    event: chat
    data: Hello World!
    data: How are you, today?

"});

let Event::Message(msg) = Decoder::default().decode(&mut body)?.unwrap() else {
    panic!("Expected a message event");
};

assert_eq!(msg.event.unwrap(), "chat");
assert_eq!(msg.data, "Hello World!\nHow are you, today?");
```

## Client integrations

Each integration provides an extension trait to read events from a response. The async clients also provide a `Manager` to receive events through a channel.

- `awc-3` for `awc` v3 extension trait and connection manager.
- `reqwest-0_12` for `reqwest` v0.12 extension trait and connection manager.
- `reqwest-0_13` for `reqwest` v0.13 extension trait and connection manager.
- `ureq-3` for `ureq` v3 blocking iterator.

<!-- cargo-rdme end -->

This crate was imported from the `russe` directory in [`robjtede/actix-web-lab`](https://github.com/robjtede/actix-web-lab/tree/dde548d/russe).
