use super::{DeserializeError, RedisClusterCommand, RedisCommand};
use crate::{slot::slot_keys, Error};

use actix::Message;
use redis_async::resp::RespValue;
use RespValue::*;

/// DEL command.
#[derive(Debug)]
pub struct Del {
    pub keys: Vec<Vec<u8>>,
}

/// DEL command, single key.
pub fn del<K: Into<Vec<u8>>>(key: K) -> Del {
    Del {
        keys: vec![key.into()],
    }
}

/// DEL command, multiple keys.
pub fn del_multiple<K: Into<Vec<u8>>, I: Iterator<Item = K>>(iter: I) -> Del {
    Del {
        keys: iter.map(Into::into).collect(),
    }
}

impl RedisCommand for Del {
    /// Integer reply: the number of keys that were removed
    type Output = i64;

    fn serialize(self) -> RespValue {
        let mut data = Vec::with_capacity(1 + self.keys.len());
        data.push("DEL".into());
        data.extend(self.keys.into_iter().map(Into::into));

        RespValue::Array(data)
    }

    fn deserialize(resp: RespValue) -> Result<Self::Output, DeserializeError> {
        match resp {
            Integer(num) => Ok(num),
            resp => Err(DeserializeError::new("invalid response to DEL", resp)),
        }
    }
}

impl RedisClusterCommand for Del {
    fn slot(&self) -> Result<u16, Vec<u16>> {
        slot_keys(self.keys.iter())
    }
}

impl Message for Del {
    type Result = Result<<Del as RedisCommand>::Output, Error>;
}
