# actix-protobuf

> Protobuf support for Actix Web.

[![crates.io](https://img.shields.io/crates/v/actix-protobuf?label=latest)](https://crates.io/crates/actix-protobuf)
[![Documentation](https://docs.rs/actix-protobuf/badge.svg?version=0.7.0)](https://docs.rs/actix-protobuf/0.7.0)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-protobuf)
[![Dependency Status](https://deps.rs/crate/actix-protobuf/0.7.0/status.svg)](https://deps.rs/crate/actix-protobuf/0.7.0)

## Documentation & Resources

- [API Documentation](https://docs.rs/actix-protobuf)
- [Example Project](https://github.com/actix/examples/tree/master/protobuf)
- Minimum Supported Rust Version (MSRV): 1.54

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

See [here](https://github.com/actix/actix-extras/tree/master/actix-protobuf/examples/prost-example) for the complete example.

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or [https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0))
- MIT license ([LICENSE-MIT](LICENSE-MIT) or [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT))

at your option.
