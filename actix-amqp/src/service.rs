use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use actix_codec::{AsyncRead, AsyncWrite, Framed};
use actix_service::Service;
use futures::{Future, SinkExt, StreamExt};

use amqp_codec::protocol::ProtocolId;
use amqp_codec::{ProtocolIdCodec, ProtocolIdError};

pub struct ProtocolNegotiation<T> {
    proto: ProtocolId,
    _r: PhantomData<T>,
}

impl<T> Clone for ProtocolNegotiation<T> {
    fn clone(&self) -> Self {
        ProtocolNegotiation {
            proto: self.proto.clone(),
            _r: PhantomData,
        }
    }
}

impl<T> ProtocolNegotiation<T> {
    pub fn new(proto: ProtocolId) -> Self {
        ProtocolNegotiation {
            proto,
            _r: PhantomData,
        }
    }
}

impl<T> Service for ProtocolNegotiation<T>
where
    T: AsyncRead + AsyncWrite + 'static,
{
    type Request = Framed<T, ProtocolIdCodec>;
    type Response = Framed<T, ProtocolIdCodec>;
    type Error = ProtocolIdError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut framed: Framed<T, ProtocolIdCodec>) -> Self::Future {
        let proto = self.proto;

        Box::pin(async move {
            framed.send(proto).await?;

            let protocol = framed.next().await.ok_or(ProtocolIdError::Disconnected)??;
            if proto == protocol {
                Ok(framed)
            } else {
                Err(ProtocolIdError::Unexpected {
                    exp: proto,
                    got: protocol,
                })
            }
        })
    }
}
