//! Redis-backed fixed window counter.

use std::time::Duration;

use redis::{AsyncConnectionConfig, Client};

use crate::errors::Error;

/// Tracks the given key in a period, returning the count and the remaining TTL for the key.
pub(crate) async fn track(
    client: &Client,
    key: &str,
    period: Duration,
) -> Result<(usize, Duration), Error> {
    let expires = period.as_secs();

    // Keep pre-redis@1 behavior by opting out of default async connection/response timeouts.
    let connection_config = AsyncConnectionConfig::new()
        .set_connection_timeout(None)
        .set_response_timeout(None);
    let mut connection = client
        .get_multiplexed_async_connection_with_config(&connection_config)
        .await?;

    // The seed of this approach is outlined Atul R in a blog post about rate limiting using
    // NodeJS and Redis. For more details, see https://blog.atulr.com/rate-limiter
    let mut pipe = redis::pipe();
    pipe.atomic()
        .cmd("SET") // Set key and value
        .arg(key)
        .arg(0)
        .arg("EX") // Set the specified expire time, in seconds.
        .arg(expires)
        .arg("NX") // Only set the key if it does not already exist.
        .ignore() // --- ignore returned value of SET command ---
        .cmd("INCR") // Increment key
        .arg(key)
        .cmd("TTL") // Return time-to-live of key
        .arg(key);

    let (count, ttl) = pipe.query_async(&mut connection).await?;

    Ok((count, Duration::from_secs(ttl)))
}
