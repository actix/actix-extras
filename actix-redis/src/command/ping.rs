use super::{DeserializeError, RedisCommand};
use crate::Error;

use actix::Message;
use redis_async::{resp::RespValue, resp_array};
use RespValue::*;

/// PING command.
#[derive(Debug)]
pub struct Ping {
    pub message: Option<Vec<u8>>,
}

/// PING command.
pub fn ping() -> Ping {
    Ping { message: None }
}

/// PING command with a message
pub fn ping_message<T: Into<Vec<u8>>>(message: T) -> Ping {
    Ping {
        message: Some(message.into()),
    }
}

impl RedisCommand for Ping {
    /// Simple string reply: the message
    type Output = String;

    fn serialize(self) -> RespValue {
        if let Some(message) = self.message {
            resp_array!["PING", message]
        } else {
            resp_array!["PING"]
        }
    }

    fn deserialize(resp: RespValue) -> Result<Self::Output, DeserializeError> {
        match resp {
            SimpleString(s) => Ok(s),
            resp => Err(DeserializeError::new("invalid response to ECHO", resp)),
        }
    }
}

impl Message for Ping {
    type Result = Result<<Ping as RedisCommand>::Output, Error>;
}
