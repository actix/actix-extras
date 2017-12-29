use std::io;
use std::collections::VecDeque;

use bytes::BytesMut;
use futures::Future;
use futures::unsync::oneshot;
use tokio_core::net::TcpStream;
use tokio_io::codec::{Decoder, Encoder};
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

#[derive(Message)]
pub struct Value(resp::RespValue);

/// Redis codec wrapper
pub struct RedisCodec;

impl Encoder for RedisCodec {
    type Item = Value;
    type Error = Error;

    fn encode(&mut self, msg: Value, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match resp::RespCodec.encode(msg.0, buf) {
            Ok(()) => Ok(()),
            Err(err) => Err(Error::Io(err))
        }
    }
}

impl Decoder for RedisCodec {
    type Item = Value;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match resp::RespCodec.decode(buf) {
            Ok(Some(item)) => Ok(Some(Value(item))),
            Ok(None) => Ok(None),
            Err(err) => Err(Error::Redis(err)),
        }
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
        RedisActor{queue: VecDeque::new()}.framed(io, RedisCodec)
    }
}

impl Actor for RedisActor {
    type Context = FramedContext<Self>;
}

impl FramedActor for RedisActor {
    type Io = TcpStream;
    type Codec = RedisCodec;
}

impl StreamHandler<Value, Error> for RedisActor {}

impl Handler<Value, Error> for RedisActor {

    fn error(&mut self, err: Error, _: &mut Self::Context) {
        if let Some(tx) = self.queue.pop_front() {
            let _ = tx.send(Err(err));
        }
    }

    fn handle(&mut self, msg: Value, _ctx: &mut Self::Context) -> Response<Self, Value> {
        if let Some(tx) = self.queue.pop_front() {
            let _ = tx.send(Ok(msg.0));
        }
        Self::empty()
    }
}

impl Handler<Command> for RedisActor {
    fn handle(&mut self, msg: Command, ctx: &mut Self::Context) -> Response<Self, Command> {
        let (tx, rx) = oneshot::channel();
        self.queue.push_back(tx);
        let _ = ctx.send(Value(msg.0));

        Self::async_reply(
            rx.map_err(|_| io::Error::new(io::ErrorKind::Other, "").into())
                .and_then(|res| res)
                .actfuture())
    }
}
