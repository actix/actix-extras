use std::io;
use std::collections::VecDeque;

use futures::Future;
use futures::unsync::oneshot;
use tokio_core::net::TcpStream;
use redis_async::{resp, error};

use actix::prelude::*;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display="Io error: {}", _0)]
    Io(io::Error),
    #[fail(display="Redis error")]
    Redis(error::Error),
}

unsafe impl Send for Error {}
unsafe impl Sync for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<error::Error> for Error {
    fn from(err: error::Error) -> Error {
        Error::Redis(err)
    }
}

pub struct Command(pub resp::RespValue);

impl ResponseType for Command {
    type Item = resp::RespValue;
    type Error = Error;
}

/// Redis comminucation actor
pub struct RedisActor {
    queue: VecDeque<oneshot::Sender<Result<resp::RespValue, Error>>>,
}

impl RedisActor {
    pub fn start(io: TcpStream) -> Address<RedisActor> {
        RedisActor{queue: VecDeque::new()}.framed(io, resp::RespCodec)
    }
}

impl Actor for RedisActor {
    type Context = FramedContext<Self>;
}

impl FramedActor for RedisActor {
    type Io = TcpStream;
    type Codec = resp::RespCodec;

    fn handle(&mut self, msg: Result<resp::RespValue, error::Error>, _ctx: &mut Self::Context) {
        if let Some(tx) = self.queue.pop_front() {
            let _ = tx.send(msg.map_err(|e| e.into()));
        }
    }
}

impl Handler<Command> for RedisActor {
    type Result = ResponseFuture<Self, Command>;

    fn handle(&mut self, msg: Command, ctx: &mut Self::Context) -> Self::Result {
        let (tx, rx) = oneshot::channel();
        self.queue.push_back(tx);
        let _ = ctx.send(msg.0);

        Box::new(
            rx.map_err(|_| io::Error::new(io::ErrorKind::Other, "").into())
                .and_then(|res| res)
                .actfuture())
    }
}
