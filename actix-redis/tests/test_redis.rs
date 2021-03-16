#[macro_use]
extern crate redis_async;

use actix_redis::{Error, RedisClient, RespValue};

#[actix_rt::test]
async fn test_error_connect() {
    let addr = RedisClient::new("localhost:54000");

    let res = addr.send(resp_array!["GET", "test"]).await;
    match res {
        Err(Error::NotConnected) => (),
        _ => panic!("Should not happen {:?}", res),
    }
}

#[actix_rt::test]
async fn test_redis() -> Result<(), Error> {
    env_logger::init();

    let addr = RedisClient::new("127.0.0.1:6379");

    let resp = addr.send(resp_array!["SET", "test", "value"]).await?;

    assert_eq!(resp, RespValue::SimpleString("OK".to_owned()));

    let resp = addr.send(resp_array!["GET", "test"]).await?;
    println!("RESP: {:?}", resp);
    assert_eq!(resp, RespValue::BulkString((&b"value"[..]).into()));
    Ok(())
}
