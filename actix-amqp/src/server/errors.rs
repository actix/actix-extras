use std::io;

use amqp_codec::{protocol, AmqpCodecError, ProtocolIdError, SaslFrame};
use bytestring::ByteString;
use derive_more::Display;

pub use amqp_codec::protocol::Error;

/// Errors which can occur when attempting to handle amqp connection.
#[derive(Debug, Display)]
pub enum ServerError<E> {
    #[display(fmt = "Message handler service error")]
    /// Message handler service error
    Service(E),
    /// Control service init error
    ControlServiceInit,
    #[display(fmt = "Amqp error: {}", _0)]
    /// Amqp error
    Amqp(AmqpError),
    #[display(fmt = "Protocol negotiation error: {}", _0)]
    /// Amqp protocol negotiation error
    Handshake(ProtocolIdError),
    /// Amqp handshake timeout
    HandshakeTimeout,
    /// Amqp codec error
    #[display(fmt = "Amqp codec error: {:?}", _0)]
    Protocol(AmqpCodecError),
    #[display(fmt = "Protocol error: {}", _0)]
    /// Amqp protocol error
    ProtocolError(Error),
    #[display(fmt = "Expected open frame, got: {:?}", _0)]
    Unexpected(protocol::Frame),
    #[display(fmt = "Unexpected sasl frame: {:?}", _0)]
    UnexpectedSaslFrame(SaslFrame),
    #[display(fmt = "Unexpected sasl frame body: {:?}", _0)]
    UnexpectedSaslBodyFrame(protocol::SaslFrameBody),
    /// Peer disconnect
    Disconnected,
    /// Unexpected io error
    Io(io::Error),
}

impl<E> Into<protocol::Error> for ServerError<E> {
    fn into(self) -> protocol::Error {
        protocol::Error {
            condition: protocol::AmqpError::InternalError.into(),
            description: Some(ByteString::from(format!("{}", self))),
            info: None,
        }
    }
}

impl<E> From<AmqpError> for ServerError<E> {
    fn from(err: AmqpError) -> Self {
        ServerError::Amqp(err)
    }
}

impl<E> From<AmqpCodecError> for ServerError<E> {
    fn from(err: AmqpCodecError) -> Self {
        ServerError::Protocol(err)
    }
}

impl<E> From<ProtocolIdError> for ServerError<E> {
    fn from(err: ProtocolIdError) -> Self {
        ServerError::Handshake(err)
    }
}

impl<E> From<SaslFrame> for ServerError<E> {
    fn from(err: SaslFrame) -> Self {
        ServerError::UnexpectedSaslFrame(err)
    }
}

impl<E> From<io::Error> for ServerError<E> {
    fn from(err: io::Error) -> Self {
        ServerError::Io(err)
    }
}

#[derive(Debug, Display)]
#[display(fmt = "Amqp error: {:?} {:?} ({:?})", err, description, info)]
pub struct AmqpError {
    err: protocol::AmqpError,
    description: Option<ByteString>,
    info: Option<protocol::Fields>,
}

impl AmqpError {
    pub fn new(err: protocol::AmqpError) -> Self {
        AmqpError {
            err,
            description: None,
            info: None,
        }
    }

    pub fn internal_error() -> Self {
        Self::new(protocol::AmqpError::InternalError)
    }

    pub fn not_found() -> Self {
        Self::new(protocol::AmqpError::NotFound)
    }

    pub fn unauthorized_access() -> Self {
        Self::new(protocol::AmqpError::UnauthorizedAccess)
    }

    pub fn decode_error() -> Self {
        Self::new(protocol::AmqpError::DecodeError)
    }

    pub fn invalid_field() -> Self {
        Self::new(protocol::AmqpError::InvalidField)
    }

    pub fn not_allowed() -> Self {
        Self::new(protocol::AmqpError::NotAllowed)
    }

    pub fn not_implemented() -> Self {
        Self::new(protocol::AmqpError::NotImplemented)
    }

    pub fn description<T: AsRef<str>>(mut self, text: T) -> Self {
        self.description = Some(ByteString::from(text.as_ref()));
        self
    }

    pub fn set_description(mut self, text: ByteString) -> Self {
        self.description = Some(text);
        self
    }
}

impl Into<protocol::Error> for AmqpError {
    fn into(self) -> protocol::Error {
        protocol::Error {
            condition: self.err.into(),
            description: self.description,
            info: self.info,
        }
    }
}

#[derive(Debug, Display)]
#[display(fmt = "Link error: {:?} {:?} ({:?})", err, description, info)]
pub struct LinkError {
    err: protocol::LinkError,
    description: Option<ByteString>,
    info: Option<protocol::Fields>,
}

impl LinkError {
    pub fn force_detach() -> Self {
        LinkError {
            err: protocol::LinkError::DetachForced,
            description: None,
            info: None,
        }
    }

    pub fn description<T: AsRef<str>>(mut self, text: T) -> Self {
        self.description = Some(ByteString::from(text.as_ref()));
        self
    }

    pub fn set_description(mut self, text: ByteString) -> Self {
        self.description = Some(text);
        self
    }
}

impl Into<protocol::Error> for LinkError {
    fn into(self) -> protocol::Error {
        protocol::Error {
            condition: self.err.into(),
            description: self.description,
            info: self.info,
        }
    }
}
