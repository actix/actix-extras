//! Redis command types.

mod del;
mod get;
mod set;

pub use del::{del, del_multiple, Del};
pub use get::{get, Get};
pub use set::{set, Set};

use redis_async::resp::RespValue;

/// The error type returned when deserializing a response from Redis faild.
pub struct DeserializeError {
    /// Error message.
    pub message: String,
    /// The RESP value (optional).
    pub resp: Option<RespValue>,
}

impl DeserializeError {
    pub fn new<S: Into<String>>(message: S, resp: RespValue) -> Self {
        DeserializeError {
            message: message.into(),
            resp: Some(resp),
        }
    }

    pub fn message<S: Into<String>>(message: S) -> Self {
        DeserializeError {
            message: message.into(),
            resp: None,
        }
    }
}

/// A Redis command.
///
/// Each command type `T` should implement `Message<Result = Result<T::Output,
/// actix_redis::Error>>` so that `RedisActor` can handle the command.
pub trait RedisCommand {
    /// The Rust type of the output of this command.
    type Output;

    /// Serialize the request into `RespValue`.
    fn serialize(self) -> RespValue;
    /// Deserialize the response from `RespValue`.
    fn deserialize(resp: RespValue) -> Result<Self::Output, DeserializeError>;
}
