use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use actix_codec::{AsyncRead, AsyncWrite, Framed};
use actix_service::{Service, ServiceFactory};
use amqp_codec::protocol::{
    self, ProtocolId, SaslChallenge, SaslCode, SaslFrameBody, SaslMechanisms, SaslOutcome, Symbols,
};
use amqp_codec::{AmqpCodec, AmqpFrame, ProtocolIdCodec, ProtocolIdError, SaslFrame};
use bytes::Bytes;
use bytestring::ByteString;
use futures::future::{err, ok, Either, Ready};
use futures::{SinkExt, StreamExt};

use super::connect::{ConnectAck, ConnectOpened};
use super::errors::{AmqpError, ServerError};
use crate::connection::ConnectionController;

pub struct Sasl<Io> {
    framed: Framed<Io, ProtocolIdCodec>,
    mechanisms: Symbols,
    controller: ConnectionController,
}

impl<Io> fmt::Debug for Sasl<Io> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SaslAuth")
            .field("mechanisms", &self.mechanisms)
            .finish()
    }
}

impl<Io> Sasl<Io> {
    pub(crate) fn new(
        framed: Framed<Io, ProtocolIdCodec>,
        controller: ConnectionController,
    ) -> Self {
        Sasl {
            framed,
            controller,
            mechanisms: Symbols::default(),
        }
    }
}

impl<Io> Sasl<Io>
where
    Io: AsyncRead + AsyncWrite,
{
    /// Returns reference to io object
    pub fn get_ref(&self) -> &Io {
        self.framed.get_ref()
    }

    /// Returns mutable reference to io object
    pub fn get_mut(&mut self) -> &mut Io {
        self.framed.get_mut()
    }

    /// Add supported sasl mechanism
    pub fn mechanism<U: Into<String>>(mut self, symbol: U) -> Self {
        self.mechanisms.push(ByteString::from(symbol.into()).into());
        self
    }

    /// Initialize sasl auth procedure
    pub async fn init(self) -> Result<Init<Io>, ServerError<()>> {
        let Sasl {
            framed,
            mechanisms,
            controller,
            ..
        } = self;

        let mut framed = framed.into_framed(AmqpCodec::<SaslFrame>::new());
        let frame = SaslMechanisms {
            sasl_server_mechanisms: mechanisms,
        }
        .into();

        framed.send(frame).await.map_err(ServerError::from)?;
        let frame = framed
            .next()
            .await
            .ok_or(ServerError::Disconnected)?
            .map_err(ServerError::from)?;

        match frame.body {
            SaslFrameBody::SaslInit(frame) => Ok(Init {
                frame,
                framed,
                controller,
            }),
            body => Err(ServerError::UnexpectedSaslBodyFrame(body)),
        }
    }
}

/// Initialization stage of sasl negotiation
pub struct Init<Io> {
    frame: protocol::SaslInit,
    framed: Framed<Io, AmqpCodec<SaslFrame>>,
    controller: ConnectionController,
}

impl<Io> fmt::Debug for Init<Io> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SaslInit")
            .field("frame", &self.frame)
            .finish()
    }
}

impl<Io> Init<Io>
where
    Io: AsyncRead + AsyncWrite,
{
    /// Sasl mechanism
    pub fn mechanism(&self) -> &str {
        self.frame.mechanism.as_str()
    }

    /// Sasl initial response
    pub fn initial_response(&self) -> Option<&[u8]> {
        self.frame.initial_response.as_ref().map(|b| b.as_ref())
    }

    /// Sasl initial response
    pub fn hostname(&self) -> Option<&str> {
        self.frame.hostname.as_ref().map(|b| b.as_ref())
    }

    /// Returns reference to io object
    pub fn get_ref(&self) -> &Io {
        self.framed.get_ref()
    }

    /// Returns mutable reference to io object
    pub fn get_mut(&mut self) -> &mut Io {
        self.framed.get_mut()
    }

    /// Initiate sasl challenge
    pub async fn challenge(self) -> Result<Response<Io>, ServerError<()>> {
        self.challenge_with(Bytes::new()).await
    }

    /// Initiate sasl challenge with challenge payload
    pub async fn challenge_with(self, challenge: Bytes) -> Result<Response<Io>, ServerError<()>> {
        let mut framed = self.framed;
        let controller = self.controller;
        let frame = SaslChallenge { challenge }.into();

        framed.send(frame).await.map_err(ServerError::from)?;
        let frame = framed
            .next()
            .await
            .ok_or(ServerError::Disconnected)?
            .map_err(ServerError::from)?;

        match frame.body {
            SaslFrameBody::SaslResponse(frame) => Ok(Response {
                frame,
                framed,
                controller,
            }),
            body => Err(ServerError::UnexpectedSaslBodyFrame(body)),
        }
    }

    /// Sasl challenge outcome
    pub async fn outcome(self, code: SaslCode) -> Result<Success<Io>, ServerError<()>> {
        let mut framed = self.framed;
        let controller = self.controller;

        let frame = SaslOutcome {
            code,
            additional_data: None,
        }
        .into();
        framed.send(frame).await.map_err(ServerError::from)?;

        Ok(Success { framed, controller })
    }
}

pub struct Response<Io> {
    frame: protocol::SaslResponse,
    framed: Framed<Io, AmqpCodec<SaslFrame>>,
    controller: ConnectionController,
}

impl<Io> fmt::Debug for Response<Io> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SaslResponse")
            .field("frame", &self.frame)
            .finish()
    }
}

impl<Io> Response<Io>
where
    Io: AsyncRead + AsyncWrite,
{
    /// Client response payload
    pub fn response(&self) -> &[u8] {
        &self.frame.response[..]
    }

    /// Sasl challenge outcome
    pub async fn outcome(self, code: SaslCode) -> Result<Success<Io>, ServerError<()>> {
        let mut framed = self.framed;
        let controller = self.controller;
        let frame = SaslOutcome {
            code,
            additional_data: None,
        }
        .into();

        framed.send(frame).await.map_err(ServerError::from)?;
        framed
            .next()
            .await
            .ok_or(ServerError::Disconnected)?
            .map_err(|res| ServerError::from(res))?;

        Ok(Success { framed, controller })
    }
}

pub struct Success<Io> {
    framed: Framed<Io, AmqpCodec<SaslFrame>>,
    controller: ConnectionController,
}

impl<Io> Success<Io>
where
    Io: AsyncRead + AsyncWrite,
{
    /// Returns reference to io object
    pub fn get_ref(&self) -> &Io {
        self.framed.get_ref()
    }

    /// Returns mutable reference to io object
    pub fn get_mut(&mut self) -> &mut Io {
        self.framed.get_mut()
    }

    /// Wait for connection open frame
    pub async fn open(self) -> Result<ConnectOpened<Io>, ServerError<()>> {
        let mut framed = self.framed.into_framed(ProtocolIdCodec);
        let mut controller = self.controller;

        let protocol = framed
            .next()
            .await
            .ok_or(ServerError::from(ProtocolIdError::Disconnected))?
            .map_err(ServerError::from)?;

        match protocol {
            ProtocolId::Amqp => {
                // confirm protocol
                framed
                    .send(ProtocolId::Amqp)
                    .await
                    .map_err(ServerError::from)?;

                // Wait for connection open frame
                let mut framed = framed.into_framed(AmqpCodec::<AmqpFrame>::new());
                let frame = framed
                    .next()
                    .await
                    .ok_or(ServerError::Disconnected)?
                    .map_err(ServerError::from)?;

                let frame = frame.into_parts().1;
                match frame {
                    protocol::Frame::Open(frame) => {
                        trace!("Got open frame: {:?}", frame);
                        controller.set_remote((&frame).into());
                        Ok(ConnectOpened::new(frame, framed, controller))
                    }
                    frame => Err(ServerError::Unexpected(frame)),
                }
            }
            proto => Err(ProtocolIdError::Unexpected {
                exp: ProtocolId::Amqp,
                got: proto,
            }
            .into()),
        }
    }
}

/// Create service factory with disabled sasl support
pub fn no_sasl<Io, St, E>() -> NoSaslService<Io, St, E> {
    NoSaslService::default()
}

pub struct NoSaslService<Io, St, E>(std::marker::PhantomData<(Io, St, E)>);

impl<Io, St, E> Default for NoSaslService<Io, St, E> {
    fn default() -> Self {
        NoSaslService(std::marker::PhantomData)
    }
}

impl<Io, St, E> ServiceFactory for NoSaslService<Io, St, E> {
    type Config = ();
    type Request = Sasl<Io>;
    type Response = ConnectAck<Io, St>;
    type Error = AmqpError;
    type InitError = E;
    type Service = NoSaslService<Io, St, E>;
    type Future = Ready<Result<Self::Service, Self::InitError>>;

    fn new_service(&self, _: ()) -> Self::Future {
        ok(NoSaslService(std::marker::PhantomData))
    }
}

impl<Io, St, E> Service for NoSaslService<Io, St, E> {
    type Request = Sasl<Io>;
    type Response = ConnectAck<Io, St>;
    type Error = AmqpError;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: Self::Request) -> Self::Future {
        err(AmqpError::not_implemented())
    }
}
