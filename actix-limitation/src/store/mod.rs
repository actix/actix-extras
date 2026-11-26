//! Rate limit counter backends.

pub(crate) mod redis;

#[cfg(feature = "memory-store")]
pub(crate) mod memory;

#[cfg(feature = "memory-store")]
pub use self::memory::{MemoryStore, MemoryStoreBuilder};

/// Counter store backend bound to a [`Limiter`](crate::Limiter).
// One `Backend` exists per application and is not cloned per request.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum Backend {
    /// Counters stored in Redis.
    Redis(::redis::Client),

    /// Counters stored in process memory.
    #[cfg(feature = "memory-store")]
    Memory(MemoryStore),
}
