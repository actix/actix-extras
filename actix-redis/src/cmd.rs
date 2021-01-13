use crate::RespValue;

/// Command for send data to Redis
#[derive(Debug)]
pub struct Command(pub RespValue);

/// Compose Redis command
#[macro_export]
macro_rules! redis_cmd {
    ($e:expr) => {
        $crate::Command(
            $crate::RespValue::BulkString($e.to_string().into_bytes())
        )
    };

    ($($e:expr),+ $(,)?) => {
        $crate::Command(crate::RespValue::Array(vec![
            $(crate::RespValue::BulkString($e.to_string().into_bytes()),)+
        ]))
    };
}
