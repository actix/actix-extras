//! Rate limit counter backends.

pub(crate) mod redis;

pub(crate) mod memory;

pub use self::memory::{MemoryStore, MemoryStoreBuilder};

/// Counter store backend bound to a [`Limiter`](crate::Limiter).
// One `Backend` exists per application and is not cloned per request.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum Backend {
    /// Counters stored in Redis.
    Redis(::redis::Client),

    /// Counters stored in process memory.
    Memory(MemoryStore),
}
