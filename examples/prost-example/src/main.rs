extern crate bytes;
extern crate actix;
extern crate actix_web;
extern crate actix_protobuf;
extern crate env_logger;
extern crate prost;
#[macro_use] 
extern crate prost_derive;

use actix_web::*;
use actix_protobuf::*;

#[derive(Clone, Debug, PartialEq, Message)]
pub struct MyObj {
    #[prost(int32, tag="1")]
    pub number: i32,
    #[prost(string, tag="2")]
    pub name: String,
}


fn index(msg: ProtoBuf<MyObj>) -> Result<HttpResponse> {
    println!("model: {:?}", msg);
    HttpResponse::Ok().protobuf(msg.0)  // <- send response
}


fn main() {
    ::std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();
    let sys = actix::System::new("prost-example");

    server::new(|| {
        App::new()
            .middleware(middleware::Logger::default())
            .resource("/", |r| r.method(http::Method::POST).with(index))})
        .bind("127.0.0.1:8080").unwrap()
        .shutdown_timeout(1)
        .start();

    println!("Started http server: 127.0.0.1:8080");
    let _ = sys.run();
}
