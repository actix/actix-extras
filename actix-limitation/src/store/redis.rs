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

    // Based on the fixed-window Redis pattern described at https://blog.atulr.com/rate-limiter.
    let mut pipe = redis::pipe();
    pipe.atomic()
        .cmd("SET")
        .arg(key)
        .arg(0)
        .arg("EX") // Set the window length in seconds.
        .arg(expires)
        .arg("NX") // Create the counter only if absent, preserving any active window.
        .ignore() // Omit the SET reply from the (count, ttl) result.
        .cmd("INCR") // Count this request.
        .arg(key)
        .cmd("TTL") // Read the seconds left in the window.
        .arg(key);

    let (count, ttl) = pipe.query_async(&mut connection).await?;

    Ok((count, Duration::from_secs(ttl)))
}
