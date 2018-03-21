# Actix-web ProtoBuf [![Build Status](https://travis-ci.org/actix/actix-protobuf.svg?branch=master)](https://travis-ci.org/actix/actix-protobuf) [![codecov](https://codecov.io/gh/actix/actix-protobuf/branch/master/graph/badge.svg)](https://codecov.io/gh/actix/actix-protobuf) [![crates.io](http://meritbadge.herokuapp.com/actix-web)](https://crates.io/crates/actix-protobuf) [![Join the chat at https://gitter.im/actix/actix](https://badges.gitter.im/actix/actix.svg)](https://gitter.im/actix/actix?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

Protobuf support for actix-web framework.


## Example

```rust,ignore
use actix_web::*;
use actix_protobuf::*;
use futures::Future;

#[derive(Clone, Debug, PartialEq, Message)]
pub struct MyObj {
    #[prost(int32, tag="1")]
    pub number: i32,
    #[prost(string, tag="2")]
    pub name: String,
}

fn index(req: HttpRequest) -> Box<Future<Item=HttpResponse, Error=Error>> {
    req.protobuf()
        .from_err()  // convert all errors into `Error`
        .and_then(|val: MyObj| {
            println!("model: {:?}", val);
            Ok(httpcodes::HTTPOk.build().protobuf(val)?)  // <- send response
        })
        .responder()
}
```
