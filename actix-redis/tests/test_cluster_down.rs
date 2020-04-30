use actix_redis::{command::*, RedisClusterActor};
use std::fmt::Debug;
use std::time::Duration;
use tokio::time::delay_for;

fn success<T: Debug, E1: Debug, E2: Debug>(res: Result<Result<T, E1>, E2>) -> T {
    match res {
        Ok(Ok(x)) => x,
        _ => panic!("Should not happen {:?}", res),
    }
}

#[actix_rt::test]
async fn test_cluster_scaledown() {
    env_logger::init();

    let addr = RedisClusterActor::start("redis.service.dc1.consul:16379");
    success(
        addr.send(Get {
            key: "shutdown".into(),
        })
        .await,
    );
    success(addr.send(set("shutdown", "1")).await);
    // 2 x cluster-node-timeout + 1
    delay_for(Duration::from_secs(3)).await;
    success(
        addr.send(Get {
            key: "shutdown".into(),
        })
        .await,
    );
}
