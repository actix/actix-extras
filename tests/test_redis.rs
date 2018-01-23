extern crate actix;
extern crate actix_redis;
#[macro_use]
extern crate redis_async;
extern crate futures;
extern crate env_logger;

use actix::prelude::*;
use actix_redis::{RedisActor, Command, Error, RespValue};
use futures::Future;

#[test]
fn test_error_connect() {
    let sys = System::new("test");

    let addr = RedisActor::start("localhost:54000");
    let _addr2 = addr.clone();

    Arbiter::handle().spawn_fn(move || {
        addr.call_fut(Command(resp_array!["GET", "test"]))
            .then(|res| {
                match res {
                    Ok(Err(Error::NotConnected)) => (),
                    _ => panic!("Should not happen {:?}", res),
                }
                Arbiter::system().send(actix::msgs::SystemExit(0));
                Ok(())
            })
    });

    sys.run();
}


#[test]
fn test_redis() {
    env_logger::init();
    let sys = System::new("test");

    let addr = RedisActor::start("127.0.0.1:6379");
    let _addr2 = addr.clone();

    Arbiter::handle().spawn_fn(move || {
        let addr2 = addr.clone();
        addr.call_fut(Command(resp_array!["SET", "test", "value"]))
            .then(move |res| match res {
                Ok(Ok(resp)) => {
                    assert_eq!(resp, RespValue::SimpleString("OK".to_owned()));
                    addr2.call_fut(Command(resp_array!["GET", "test"]))
                        .then(|res| {
                            match res {
                                Ok(Ok(resp)) => {
                                    println!("RESP: {:?}", resp);
                                    assert_eq!(
                                        resp, RespValue::BulkString((&b"value"[..]).into()));
                                },
                                _ => panic!("Should not happen {:?}", res),
                            }
                            Arbiter::system().send(actix::msgs::SystemExit(0));
                            Ok(())
                        })
                },
                _ => panic!("Should not happen {:?}", res),
            })
    });

    sys.run();
}
