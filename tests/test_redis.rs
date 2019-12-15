#[macro_use]
extern crate redis_async;

use actix_redis::{Command, Error, RedisActor, RespValue};

#[actix_rt::test]
async fn test_error_connect() {
    let addr = RedisActor::start("localhost:54000");
    let _addr2 = addr.clone();

    let res = addr.send(Command(resp_array!["GET", "test"])).await;
    match res {
        Ok(Err(Error::NotConnected)) => (),
        _ => panic!("Should not happen {:?}", res),
    }
}

#[actix_rt::test]
async fn test_redis() {
    env_logger::init();

    let addr = RedisActor::start("127.0.0.1:6379");
    let res = addr
        .send(Command(resp_array!["SET", "test", "value"]))
        .await;

    match res {
        Ok(Ok(resp)) => {
            assert_eq!(resp, RespValue::SimpleString("OK".to_owned()));

            let res = addr.send(Command(resp_array!["GET", "test"])).await;
            match res {
                Ok(Ok(resp)) => {
                    println!("RESP: {:?}", resp);
                    assert_eq!(resp, RespValue::BulkString((&b"value"[..]).into()));
                }
                _ => panic!("Should not happen {:?}", res),
            }
        }
        _ => panic!("Should not happen {:?}", res),
    }
}
