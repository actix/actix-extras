use super::{DeserializeError, RedisCommand};
use crate::Error;

use actix::Message;
use redis_async::{resp::RespValue, resp_array};
use RespValue::*;

/// ECHO command.
#[derive(Debug)]
pub struct Echo {
    pub message: Vec<u8>,
}

/// ECHO command.
pub fn echo<T: Into<Vec<u8>>>(message: T) -> Echo {
    Echo {
        message: message.into(),
    }
}

impl RedisCommand for Echo {
    /// Bulk string reply: the message
    type Output = Vec<u8>;

    fn serialize(self) -> RespValue {
        resp_array!["ECHO", self.message]
    }

    fn deserialize(resp: RespValue) -> Result<Self::Output, DeserializeError> {
        match resp {
            BulkString(s) => Ok(s),
            resp => Err(DeserializeError::new("invalid response to ECHO", resp)),
        }
    }
}

impl Message for Echo {
    type Result = Result<<Echo as RedisCommand>::Output, Error>;
}
