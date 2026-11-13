# Actix SSE

[<img alt="github" src="https://img.shields.io/badge/github-actix/actix_extras-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/actix/actix-extras/tree/main/actix-sse) [<img alt="crates.io" src="https://img.shields.io/crates/v/actix-sse?color=fc8d62&logo=rust&style=for-the-badge" height="20">](https://crates.io/crates/actix-sse)

SSE implementation for Actix, extracted from [actix-web-lab](https://github.com/robjtede/actix-web-lab/) with minimal dependencies.

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

This should mostly be a drop-in replacement for the `sse` module, but we did remove a few convenience methods.

- `Data::from_json`: Serialize in the caller and use `Data::new`
- `Data::set_id`: Use `Data::id`
- `Sse::from_receiver`: Use `Sse::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))`
