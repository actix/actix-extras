# Actix SSE

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-sse?label=latest)](https://crates.io/crates/actix-sse)
[![Documentation](https://docs.rs/actix-sse/badge.svg?version=0.3.0)](https://docs.rs/actix-sse/0.3.0)
![Version](https://img.shields.io/badge/rustc-1.89+-ab6000.svg)
![MIT or Apache 2.0 licensed](https://img.shields.io/crates/l/actix-sse.svg)
<br />
[![Dependency Status](https://deps.rs/crate/actix-sse/0.3.0/status.svg)](https://deps.rs/crate/actix-sse/0.3.0)
[![Download](https://img.shields.io/crates/d/actix-sse.svg)](https://crates.io/crates/actix-sse)
[![Chat on Discord](https://img.shields.io/discord/771444961383153695?label=chat&logo=discord)](https://discord.gg/NWpN5mmg3x)

<!-- prettier-ignore-end -->

SSE responder for Actix Web, with event encoding provided by `russe`.

The implementation and licenses were imported from the [SSE module in `actix-web-lab`](https://github.com/robjtede/actix-web-lab/blob/dde548d6cacdeb9606e66c4a19a8bdc5f99171b8/actix-web-lab/src/sse.rs).

```rust
use std::{convert::Infallible, time::Duration};

#[get("/from-stream")]
async fn from_stream() -> impl Responder {
    let event_stream = futures_util::stream::iter([Ok::<_, Infallible>(actix_sse::Event::Data(
        actix_sse::Data::new("foo"),
    ))]);

    actix_sse::Sse::from_stream(event_stream).with_keep_alive(Duration::from_secs(5))
}
```

## Migrating from actix-web-lab

Replace `use actix_web_lab::sse;` with `use actix_sse as sse;`.

- For JSON data, serialize in the caller and pass the result to `Data::new`. The `Data::from_json` / `Data::new_json` helper is not available.
- Replace `Data::set_id` with the `Data::id` builder method.
- Replace `Sse::from_receiver(rx)` with `Sse::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))`.
