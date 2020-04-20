use super::{DeserializeError, RedisCommand};
use crate::Error;

use actix::Message;
use redis_async::{resp::RespValue, resp_array};
use RespValue::*;

/// GET command.
#[derive(Debug)]
pub struct Get {
    pub key: Vec<u8>,
}

/// GET command.
pub fn get<K: Into<Vec<u8>>>(key: K) -> Get {
    Get { key: key.into() }
}

impl RedisCommand for Get {
    /// Bulk string reply:
    /// - `Some(s)` where `s` is the value of key; or
    /// - `None` when key does not exist.
    type Output = Option<Vec<u8>>;

    fn serialize(self) -> RespValue {
        resp_array!["GET", self.key]
    }

    fn deserialize(resp: RespValue) -> Result<Self::Output, DeserializeError> {
        match resp {
            Nil => Ok(None),
            BulkString(s) => Ok(Some(s)),
            resp => Err(DeserializeError::new("invalid response to GET", resp)),
        }
    }
}

impl Message for Get {
    type Result = Result<<Get as RedisCommand>::Output, Error>;
}
