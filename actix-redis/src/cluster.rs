use actix::prelude::*;
use futures::future::FutureExt;
use redis_async::resp::RespValue;

use std::collections::HashMap;

use crate::command::{Asking, ClusterSlots, RedisClusterCommand, RedisCommand};
use crate::{Error, RedisActor, RespError, Slots};

const MAX_RETRY: usize = 16;

// Formats RESP value in UTF-8 (lossy).
struct DebugResp<'a>(&'a RespValue);

impl std::fmt::Debug for DebugResp<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.0 {
            RespValue::Nil => write!(f, "nil"),
            RespValue::Integer(n) => write!(f, "{:?}", n),
            RespValue::Array(o) => {
                f.debug_list().entries(o.iter().map(DebugResp)).finish()
            }
            RespValue::SimpleString(s) => write!(f, "{:?}", s),
            RespValue::BulkString(s) => write!(f, "{:?}", String::from_utf8_lossy(s)),
            RespValue::Error(e) => write!(f, "{:?}", e),
        }
    }
}

pub struct RedisClusterActor {
    initial_addr: String,
    slots: Vec<Slots>,
    connections: HashMap<String, Addr<RedisActor>>,
}

impl RedisClusterActor {
    pub fn start<S: Into<String>>(addr: S) -> Addr<RedisClusterActor> {
        let addr = addr.into();

        Supervisor::start(move |_ctx| RedisClusterActor {
            initial_addr: addr,
            slots: vec![],
            connections: HashMap::new(),
        })
    }

    fn refresh_slots(&mut self) -> ResponseActFuture<Self, ()> {
        let addr = self.initial_addr.clone();
        let control_connection = self
            .connections
            .entry(addr.clone())
            .or_insert_with(move || RedisActor::start(addr));

        Box::pin(
            control_connection
                .send(ClusterSlots)
                .map(|res| match res {
                    Ok(Ok(slots)) => Ok(slots),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(Error::Disconnected),
                })
                .into_actor(self)
                .map(|res, this, _ctx| match res {
                    Ok(slots) => {
                        for slots in slots.iter() {
                            this.connections
                                .entry(slots.master_addr().to_string())
                                .or_insert_with(|| {
                                    RedisActor::start(slots.master_addr())
                                });
                        }
                        this.slots = slots;
                        debug!("slots: {:?}", this.slots);
                    }
                    Err(e) => {
                        warn!("refreshing slots failed: {:?}", e);
                    }
                }),
        )
    }
}

impl Actor for RedisClusterActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.wait(self.refresh_slots());
    }
}

impl Supervised for RedisClusterActor {
    fn restarting(&mut self, _: &mut Self::Context) {
        self.slots.clear();
        self.connections.clear();
    }
}

#[derive(Debug, Clone)]
struct Retry {
    addr: String,
    req: RespValue,
    retry: usize,
}

impl Message for Retry {
    type Result = Result<RespValue, Error>;
}

impl Retry {
    fn new(addr: String, req: RespValue, retry: usize) -> Self {
        Retry { addr, req, retry }
    }
}

impl Handler<Retry> for RedisClusterActor {
    type Result = ResponseActFuture<RedisClusterActor, Result<RespValue, Error>>;

    fn handle(&mut self, msg: Retry, _ctx: &mut Self::Context) -> Self::Result {
        fn do_retry(
            this: &mut RedisClusterActor,
            addr: String,
            req: RespValue,
            retry: usize,
        ) -> ResponseActFuture<RedisClusterActor, Result<RespValue, Error>> {
            use actix::fut::{err, ok};

            debug!(
                "processing: addr = {}, retry = {}, request = {:?}",
                addr,
                retry,
                DebugResp(&req)
            );

            let connection = this
                .connections
                .entry(addr.clone())
                .or_insert_with(move || RedisActor::start(addr));
            Box::pin(
                connection
                    .send(crate::redis::Command(req.clone()))
                    .into_actor(this)
                    .then(move |res, this, ctx| {
                        debug!(
                            "received: {:?}",
                            res.as_ref().map(|res| res.as_ref().map(DebugResp))
                        );
                        match res {
                            Ok(Ok(RespValue::Error(ref e)))
                                if e.starts_with("MOVED") && retry < MAX_RETRY =>
                            {
                                info!(
                                    "MOVED redirection: retry = {}, request = {:?}",
                                    retry,
                                    DebugResp(&req)
                                );

                                let mut values = e.split(' ');
                                let _moved = values.next().unwrap();
                                let _slot = values.next().unwrap();
                                let addr = values.next().unwrap();

                                ctx.wait(this.refresh_slots());

                                do_retry(this, addr.to_string(), req, retry + 1)
                            }
                            Ok(Ok(RespValue::Error(ref e)))
                                if e.starts_with("ASK") && retry < MAX_RETRY =>
                            {
                                info!(
                                    "ASK redirection: retry = {}, request = {:?}",
                                    retry,
                                    DebugResp(&req)
                                );

                                let mut values = e.split(' ');
                                let _moved = values.next().unwrap();
                                let _slot = values.next().unwrap();
                                let addr = values.next().unwrap();

                                ctx.spawn(
                                    // No retry for ASKING
                                    do_retry(
                                        this,
                                        addr.to_string(),
                                        Asking.serialize(),
                                        MAX_RETRY,
                                    )
                                    .map(
                                        |res, _this, _ctx| {
                                            match res.map(Asking::deserialize) {
                                                Ok(Ok(())) => {}
                                                e => warn!(
                                                    "failed to issue ASKING: {:?}",
                                                    e
                                                ),
                                            };
                                        },
                                    ),
                                );

                                do_retry(this, addr.to_string(), req, retry + 1)
                            }
                            Ok(Ok(res)) => Box::pin(ok(res)),
                            Ok(Err(e)) => Box::pin(err(e)),
                            Err(_canceled) => Box::pin(err(Error::Disconnected)),
                        }
                    }),
            )
        }

        do_retry(self, msg.addr, msg.req, msg.retry)
    }
}

impl<T> Handler<T> for RedisClusterActor
where
    T: RedisClusterCommand
        + Message<Result = Result<<T as RedisCommand>::Output, Error>>,
    T::Output: Send + 'static,
{
    type Result = ResponseActFuture<RedisClusterActor, Result<T::Output, Error>>;

    fn handle(&mut self, msg: T, ctx: &mut Self::Context) -> Self::Result {
        // refuse operations over multiple slots
        let slot = match msg.slot() {
            Ok(slot) => slot,
            Err(e) => return Box::pin(actix::fut::err(Error::DifferentSlots(e))),
        };
        let req = msg.serialize();

        let fut = if let Some(slots) = self
            .slots
            .iter()
            .find(|slots| slots.start <= slot && slot <= slots.end)
        {
            let addr = slots.master_addr();
            actix::Handler::handle(self, Retry::new(addr, req, 0), ctx)
        } else {
            warn!("no node is serving the slot {}", slot);
            Box::pin(actix::fut::err(Error::NotConnected))
        };

        Box::pin(fut.map(|res, _this, _ctx| {
            match res {
                Ok(res) => T::deserialize(res)
                    .map_err(|e| Error::Redis(RespError::RESP(e.message, e.resp))),
                Err(e) => Err(e),
            }
        }))
    }
}
