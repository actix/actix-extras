use super::{DeserializeError, RedisCommand};
use crate::Error;

use actix::Message;
use redis_async::{resp::RespValue, resp_array};

#[derive(Debug)]
pub struct Asking;

pub fn asking() -> Asking {
    Asking
}

impl RedisCommand for Asking {
    type Output = ();

    fn serialize(self) -> RespValue {
        resp_array!["ASKING"]
    }

    fn deserialize(resp: RespValue) -> Result<Self::Output, DeserializeError> {
        match resp {
            RespValue::SimpleString(s) if s == "OK" => Ok(()),
            resp => Err(DeserializeError::new("invalid response to ASKING", resp)),
        }
    }
}

impl Message for Asking {
    type Result = Result<(), Error>;
}
