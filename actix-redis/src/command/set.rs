use super::{DeserializeError, RedisClusterCommand, RedisCommand};
use crate::{slot::slot, Error};

use actix::Message;
use redis_async::resp::RespValue;
use RespValue::*;

/// SET commmand.
#[derive(Debug)]
pub struct Set {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub ex: Option<i64>,
    pub px: Option<i64>,
    pub nx: bool,
    pub xx: bool,
    pub keep_ttl: bool,
}

impl Set {
    /// Set timeout associated with the key, in seconds.
    pub fn ex(self, ex: i64) -> Self {
        Set {
            ex: Some(ex),
            ..self
        }
    }

    /// Set timeout associated with the key, in milliseconds.
    pub fn px(self, px: i64) -> Self {
        Set {
            px: Some(px),
            ..self
        }
    }

    /// Insert the key only if it does not exist yet.
    pub fn nx(mut self) -> Self {
        self.nx = true;
        self
    }

    /// Insert the key only if it already exists.
    pub fn xx(mut self) -> Self {
        self.xx = true;
        self
    }

    /// Retain the time to live associated with the key.
    pub fn keep_ttl(mut self) -> Self {
        self.keep_ttl = true;
        self
    }
}

/// SET command with the specified key and value.
///
/// To set the attributes (e.g. TTL) of the command, please see [`Set`](struct.Set.html#methods).
pub fn set<K: Into<Vec<u8>>, V: Into<Vec<u8>>>(key: K, value: V) -> Set {
    Set {
        key: key.into(),
        value: value.into(),
        ex: None,
        px: None,
        nx: false,
        xx: false,
        keep_ttl: false,
    }
}

impl RedisCommand for Set {
    /// - `true` if SET was executed successfully
    /// - `false` if SET was not executed due to `NX`/`XX` conditions didn't met
    type Output = bool;

    fn serialize(self) -> RespValue {
        let mut data = vec!["SET".into(), self.key.into(), self.value.into()];

        if let Some(ex) = self.ex {
            data.push("EX".into());
            data.push(ex.to_string().into());
        }

        if let Some(px) = self.px {
            data.push("PX".into());
            data.push(px.to_string().into());
        }

        if self.nx {
            data.push("NX".into());
        }

        if self.xx {
            data.push("XX".into());
        }

        if self.keep_ttl {
            data.push("KEEPTTL".into());
        }

        RespValue::Array(data)
    }

    fn deserialize(resp: RespValue) -> Result<Self::Output, DeserializeError> {
        match resp {
            Nil => Ok(false),
            SimpleString(s) if s == "OK" => Ok(true),
            resp => Err(DeserializeError::new("invalid response to SET", resp)),
        }
    }
}

impl RedisClusterCommand for Set {
    fn slot(&self) -> Result<u16, Vec<u16>> {
        Ok(slot(&self.key))
    }
}

impl Message for Set {
    type Result = Result<<Set as RedisCommand>::Output, Error>;
}
