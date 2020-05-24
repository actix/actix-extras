# actix-protobuf

[![crates.io](https://img.shields.io/crates/v/actix-protobuf)](https://crates.io/crates/actix-protobuf)
[![Documentation](https://docs.rs/actix-protobuf/badge.svg)](https://docs.rs/actix-protobuf)
[![Dependency Status](https://deps.rs/crate/actix-protobuf/0.5.1/status.svg)](https://deps.rs/crate/actix-protobuf/0.5.1)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-protobuf)
[![Join the chat at https://gitter.im/actix/actix](https://badges.gitter.im/actix/actix.svg)](https://gitter.im/actix/actix?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

> Protobuf support for actix-web framework.

* Minimum supported Rust version: 1.40.0 or later

## Example

```rust,ignore
use actix_protobuf::*;
use actix_web::*;

#[derive(Clone, PartialEq, Message)]
pub struct MyObj {
    #[prost(int32, tag = "1")]
    pub number: i32,
    #[prost(string, tag = "2")]
    pub name: String,
}

async fn index(msg: ProtoBuf<MyObj>) -> Result<HttpResponse> {
    println!("model: {:?}", msg);
    HttpResponse::Ok().protobuf(msg.0) // <- send response
}
```

See [here](https://github.com/actix/actix-protobuf/tree/master/examples/prost-example) for the complete example.

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or [https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) or [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT))

at your option.
