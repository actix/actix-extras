use amqp_codec::{protocol, AmqpCodecError, ProtocolIdError};

#[derive(Debug, Display, Clone)]
pub enum AmqpTransportError {
    Codec(AmqpCodecError),
    TooManyChannels,
    Disconnected,
    Timeout,
    #[display(fmt = "Connection closed, error: {:?}", _0)]
    Closed(Option<protocol::Error>),
    #[display(fmt = "Session ended, error: {:?}", _0)]
    SessionEnded(Option<protocol::Error>),
    #[display(fmt = "Link detached, error: {:?}", _0)]
    LinkDetached(Option<protocol::Error>),
}

impl From<AmqpCodecError> for AmqpTransportError {
    fn from(err: AmqpCodecError) -> Self {
        AmqpTransportError::Codec(err)
    }
}

#[derive(Debug, Display, From)]
pub enum SaslConnectError {
    Protocol(ProtocolIdError),
    AmqpError(AmqpCodecError),
    #[display(fmt = "Sasl error code: {:?}", _0)]
    Sasl(protocol::SaslCode),
    ExpectedOpenFrame,
    Disconnected,
}
