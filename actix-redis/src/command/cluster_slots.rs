use super::{DeserializeError, RedisCommand};
use crate::{Error, Slots};

use actix::Message;
use redis_async::{resp::RespValue, resp_array};
use RespValue::*;

#[derive(Debug)]
pub struct ClusterSlots;

pub fn cluster_slots() -> ClusterSlots {
    ClusterSlots
}

impl RedisCommand for ClusterSlots {
    type Output = Vec<Slots>;

    fn serialize(self) -> RespValue {
        resp_array!["CLUSTER", "SLOTS"]
    }

    fn deserialize(resp: RespValue) -> Result<Self::Output, DeserializeError> {
        // FromResp returns redis_async::Error, so we need our own version of conversions here
        fn parse_int(resp: RespValue) -> Result<i64, DeserializeError> {
            match resp {
                Integer(i) => Ok(i),
                resp => {
                    Err(DeserializeError::new("CLUSTER SLOTS: not an integer", resp))
                }
            }
        }

        fn parse_string(resp: RespValue) -> Result<String, DeserializeError> {
            match resp {
                SimpleString(s) => Ok(s),
                BulkString(s) => Ok(String::from_utf8_lossy(&s).into()),
                resp => Err(DeserializeError::new("CLUSTER SLOTS: not a string", resp)),
            }
        }

        fn parse_entry(resp: RespValue) -> Result<Slots, DeserializeError> {
            use std::convert::TryInto;

            match resp {
                Array(values) if values.len() >= 3 => {
                    let mut it = values.into_iter();
                    let start: u16 = parse_int(it.next().unwrap())?.try_into().unwrap();
                    let end: u16 = parse_int(it.next().unwrap())?.try_into().unwrap();

                    let nodes = it
                        .map(|node| match node {
                            Array(node) if node.len() >= 2 => {
                                let mut it = node.into_iter();
                                let addr = parse_string(it.next().unwrap())?;
                                let port = parse_int(it.next().unwrap())?;
                                let id =
                                    it.next().and_then(|resp| parse_string(resp).ok());

                                Ok((addr, port, id))
                            }
                            node => Err(DeserializeError::new(
                                "invalid node entry in response to CLUSTER SLOTS",
                                node,
                            )),
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    Ok(Slots { start, end, nodes })
                }
                resp => Err(DeserializeError::new(
                    "invalid response to CLUSTER SLOTS",
                    resp,
                )),
            }
        }

        match resp {
            Array(entries) => entries
                .into_iter()
                .map(parse_entry)
                .collect::<Result<Vec<_>, _>>(),
            resp => Err(DeserializeError::new(
                "invalid response to CLUSTER SLOTS",
                resp,
            )),
        }
    }
}

impl Message for ClusterSlots {
    type Result = Result<Vec<Slots>, Error>;
}
