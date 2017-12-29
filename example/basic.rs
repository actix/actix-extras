#![allow(unused_variables)]
#![cfg_attr(feature="cargo-clippy", allow(needless_pass_by_value))]

extern crate actix;
extern crate actix_web;
extern crate actix_redis;
extern crate env_logger;
extern crate futures;

use actix_web::*;
use actix_web::middleware::RequestSession;
use actix_redis::RedisSessionBackend;


/// simple handler
fn index(mut req: HttpRequest) -> Result<HttpResponse> {
    println!("{:?}", req);
    if let Ok(ch) = req.payload_mut().readany().poll() {
        if let futures::Async::Ready(Some(d)) = ch {
            println!("{}", String::from_utf8_lossy(d.as_ref()));
        }
    }

    // session
    if let Some(count) = req.session().get::<i32>("counter")? {
        println!("SESSION value: {}", count);
        req.session().set("counter", count+1)?;
    } else {
        req.session().set("counter", 1)?;
    }

    Ok("Welcome!".into())
}

fn main() {
    ::std::env::set_var("RUST_LOG", "actix_web=info");
    let _ = env_logger::init();
    let sys = actix::System::new("basic-example");

    HttpServer::new(
        || Application::new()
            // enable logger
            .middleware(middleware::Logger::default())
            // cookie session middleware
            .middleware(middleware::SessionStorage::new(
                RedisSessionBackend::new("127.0.0.1:6379", Duration::from_secs(7200))
                    .expect("Can not connect to redis server")
            ))
            // register simple route, handle all methods
            .resource("/", |r| r.f(index)))
        .bind("0.0.0.0:8080").unwrap()
        .start();

    println!("Starting http server: 127.0.0.1:8080");
    let _ = sys.run();
}
