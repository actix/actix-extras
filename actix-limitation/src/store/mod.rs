//! Rate limit counter backends.

pub(crate) mod redis;

#[cfg(feature = "memory-store")]
pub(crate) mod memory;

#[cfg(feature = "memory-store")]
pub use self::memory::{MemoryStore, MemoryStoreBuilder};

/// The counter store a [`Limiter`](crate::Limiter) is bound to.
///
/// A limiter holds exactly one variant, chosen at construction time, so using two backends at once
/// is unrepresentable.
// one `Backend` exists per application and is never cloned per request, so the padding is free
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum Backend {
    /// Counters kept in a Redis server.
    Redis(::redis::Client),

    /// Counters kept in this process's memory.
    #[cfg(feature = "memory-store")]
    Memory(MemoryStore),
}
