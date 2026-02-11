use actix_web::web::Bytes;
use bytestring::ByteString;
use serde::{de::DeserializeOwned, Serialize};

use super::{CodecMessage, EncodedMessage, MessageCodec};
use crate::AggregatedMessage;

/// JSON codec using `serde_json`.
///
/// By default, values are encoded as text frames.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonCodec {
    send_mode: JsonSendMode,
}

#[derive(Debug, Clone, Copy, Default)]
enum JsonSendMode {
    #[default]
    Text,
    Binary,
}

impl JsonCodec {
    /// Constructs a new JSON codec.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Encodes outgoing values as WebSocket text frames (the default).
    #[must_use]
    pub fn text(mut self) -> Self {
        self.send_mode = JsonSendMode::Text;
        self
    }

    /// Encodes outgoing values as WebSocket binary frames.
    ///
    /// Incoming frames are still accepted as either text or binary.
    #[must_use]
    pub fn binary(mut self) -> Self {
        self.send_mode = JsonSendMode::Binary;
        self
    }
}

impl<T> MessageCodec<T> for JsonCodec
where
    T: Serialize + DeserializeOwned,
{
    type Error = serde_json::Error;

    fn encode(&self, item: &T) -> Result<EncodedMessage, Self::Error> {
        match self.send_mode {
            JsonSendMode::Text => {
                let json = serde_json::to_string(item)?;
                Ok(EncodedMessage::Text(ByteString::from(json)))
            }
            JsonSendMode::Binary => {
                let json = serde_json::to_vec(item)?;
                Ok(EncodedMessage::Binary(Bytes::from(json)))
            }
        }
    }

    fn decode(&self, msg: AggregatedMessage) -> Result<CodecMessage<T>, Self::Error> {
        match msg {
            AggregatedMessage::Text(text) => {
                serde_json::from_str(text.as_ref()).map(CodecMessage::Item)
            }
            AggregatedMessage::Binary(bin) => serde_json::from_slice(&bin).map(CodecMessage::Item),
            AggregatedMessage::Ping(bytes) => Ok(CodecMessage::Ping(bytes)),
            AggregatedMessage::Pong(bytes) => Ok(CodecMessage::Pong(bytes)),
            AggregatedMessage::Close(reason) => Ok(CodecMessage::Close(reason)),
        }
    }
}
