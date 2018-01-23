use std::io;
use std::collections::VecDeque;

use actix::prelude::*;
use backoff::ExponentialBackoff;
use backoff::backoff::Backoff;
use futures::Future;
use futures::unsync::oneshot;
use tokio_io::AsyncRead;
use tokio_core::net::TcpStream;
use redis_async::error::Error as RespError;
use redis_async::resp::{RespCodec, RespValue};

use Error;
use connect::TcpConnector;

#[derive(Message, Debug)]
#[rtype(RespValue, Error)]
pub struct Command(pub RespValue);

/// Redis comminucation actor
pub struct RedisActor {
    addr: String,
    backoff: ExponentialBackoff,
    cell: Option<FramedCell<RedisActor>>,
    queue: VecDeque<oneshot::Sender<Result<RespValue, Error>>>,
}

impl RedisActor {
    pub fn start<S: Into<String>>(addr: S) -> Address<RedisActor> {
        let addr = addr.into();

        Supervisor::start(|_| {
            RedisActor { addr: addr,
                         cell: None,
                         backoff: ExponentialBackoff::default(),
                         queue: VecDeque::new() }
        })
    }
}

impl Actor for RedisActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        TcpConnector::new(self.addr.as_str())
            .into_actor(self)
            .map(|stream, act, ctx| {
                info!("Connected to redis server: {}", act.addr);
                act.backoff.reset();
                act.cell = Some(act.add_framed(stream.framed(RespCodec), ctx));
            })
            .map_err(|err, act, ctx| {
                error!("Can not connect to redis server: {}", err);
                debug!("{:?}", err);
                // re-connect with backoff time.
                // we stop currect context, supervisor will restart it.
                if let Some(timeout) = act.backoff.next_backoff() {
                    ctx.run_later(timeout, |_, ctx| ctx.stop());
                } else {
                    ctx.stop();
                }
            })
            .wait(ctx);
    }
}

impl Supervised for RedisActor {
    fn restarting(&mut self, _: &mut Self::Context) {
        self.cell.take();
        for tx in self.queue.drain(..) {
            let _ = tx.send(Err(Error::Disconnected));
        }
    }
}

impl FramedActor for RedisActor {
    type Io = TcpStream;
    type Codec = RespCodec;

    fn closed(&mut self, error: Option<io::Error>, _: &mut Self::Context) {
        if let Some(err) = error {
            warn!("Redis connection dropped: {} error: {}", self.addr, err);
        } else {
            warn!("Redis connection dropped: {}", self.addr);
        }
    }

    fn handle(&mut self, msg: Result<RespValue, RespError>, _ctx: &mut Self::Context) {
        if let Some(tx) = self.queue.pop_front() {
            let _ = tx.send(msg.map_err(|e| e.into()));
        }
    }
}

impl Handler<Command> for RedisActor {
    type Result = ResponseFuture<Self, Command>;

    fn handle(&mut self, msg: Command, _: &mut Self::Context) -> Self::Result {
        let (tx, rx) = oneshot::channel();
        if let Some(ref mut cell) = self.cell {
            self.queue.push_back(tx);
            cell.send(msg.0);
        } else {
            let _ = tx.send(Err(Error::NotConnected));
        }

        Box::new(
            rx.map_err(|_| Error::Disconnected)
                .and_then(|res| res)
                .actfuture())
    }
}
