#[macro_use]
extern crate redis_async;

use actix::prelude::*;
use actix_redis::{Command, Error, RedisActor, RespValue};
use futures::Future;

#[test]
fn test_error_connect() -> std::io::Result<()> {
    let sys = System::new("test");

    let addr = RedisActor::start("localhost:54000");
    let _addr2 = addr.clone();

    Arbiter::spawn_fn(move || {
        addr.send(Command(resp_array!["GET", "test"])).then(|res| {
            match res {
                Ok(Err(Error::NotConnected)) => (),
                _ => panic!("Should not happen {:?}", res),
            }
            System::current().stop();
            Ok(())
        })
    });

    sys.run()
}

#[test]
fn test_redis() -> std::io::Result<()> {
    env_logger::init();
    let sys = System::new("test");

    let addr = RedisActor::start("127.0.0.1:6379");
    let _addr2 = addr.clone();

    Arbiter::spawn_fn(move || {
        let addr2 = addr.clone();
        addr.send(Command(resp_array!["SET", "test", "value"]))
            .then(move |res| match res {
                Ok(Ok(resp)) => {
                    assert_eq!(resp, RespValue::SimpleString("OK".to_owned()));
                    addr2.send(Command(resp_array!["GET", "test"])).then(|res| {
                        match res {
                            Ok(Ok(resp)) => {
                                println!("RESP: {:?}", resp);
                                assert_eq!(
                                    resp,
                                    RespValue::BulkString((&b"value"[..]).into())
                                );
                            }
                            _ => panic!("Should not happen {:?}", res),
                        }
                        System::current().stop();
                        Ok(())
                    })
                }
                _ => panic!("Should not happen {:?}", res),
            })
    });

    sys.run()
}
