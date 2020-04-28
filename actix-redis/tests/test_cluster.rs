use actix_redis::{command::*, RedisClusterActor};

#[actix_rt::test]
async fn test_cluster() {
    env_logger::init();

    let addr = RedisClusterActor::start("127.0.0.1:7000");

    let res = addr.send(set("test", "value")).await;

    match res {
        Ok(Ok(true)) => {
            let res = addr.send(get("test")).await;
            match res {
                Ok(Ok(Some(resp))) => {
                    assert_eq!(resp, b"value");
                }
                _ => panic!("Should not happen {:?}", res),
            }
        }
        _ => panic!("Should not happen {:?}", res),
    }
}
