use std::{
    collections::HashMap,
    fmt,
    num::NonZeroUsize,
    sync::{Arc, Mutex, PoisonError, TryLockError},
    time::{Duration, Instant},
};

/// Default interval between lazy sweeps of expired counters.
const DEFAULT_SWEEP_INTERVAL: Duration = Duration::from_secs(60);

/// A process-local store for fixed-window counters.
///
/// Clones share counters. Create one store outside `HttpServer::new` to share it across workers.
/// Each application process has its own counters and loses them on restart.
///
/// The first request for a key starts its window. Later requests increment the count until the
/// window expires. This store supports sub-second periods. Use whole seconds if the same
/// configuration must also work with Redis.
///
/// Writes sweep expired entries once the 60-second sweep interval has elapsed. A new key also
/// triggers a sweep when the store is full. If all entries are still active, the new key passes
/// untracked. The default capacity is `usize::MAX`. Set a smaller limit with
/// [`max_keys`](MemoryStoreBuilder::max_keys).
///
/// ```
/// use actix_limitation::{Limiter, MemoryStore};
///
/// let store = MemoryStore::builder().max_keys(10_000).build();
/// let limiter = Limiter::memory_builder(store).build().unwrap();
/// ```
#[derive(Clone)]
pub struct MemoryStore {
    inner: Arc<Inner>,
}

struct Inner {
    max_keys: NonZeroUsize,
    sweep_interval: Duration,
    state: Mutex<State>,
}

struct State {
    counters: HashMap<String, Counter>,
    next_sweep: Instant,
    capacity_warned_at: Option<Instant>,
}

#[derive(Clone, Copy)]
struct Counter {
    count: usize,
    expires_at: Instant,
}

impl MemoryStore {
    /// Constructs an in-memory store.
    ///
    /// Use [`MemoryStore::builder()`] to configure it.
    ///
    /// Defaults to `usize::MAX` keys and a 60-second sweep interval.
    #[must_use]
    pub fn new() -> Self {
        MemoryStore::builder().build()
    }

    /// Constructs a [`MemoryStoreBuilder`].
    #[must_use]
    pub fn builder() -> MemoryStoreBuilder {
        MemoryStoreBuilder {
            max_keys: NonZeroUsize::MAX,
            sweep_interval: DEFAULT_SWEEP_INTERVAL,
        }
    }

    /// Tracks the given key in a period, returning the count and remaining window duration.
    pub(crate) fn track(&self, key: &str, period: Duration) -> (usize, Duration) {
        let now = Instant::now();

        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        if now >= state.next_sweep {
            state.counters.retain(|_, counter| counter.expires_at > now);
            state.next_sweep = now + self.inner.sweep_interval;
        }

        let counter = state.counters.get(key).copied();

        let start_new_window = counter.is_none_or(|counter| counter.expires_at <= now);

        let max_keys = self.inner.max_keys.get();
        if counter.is_none() && state.counters.len() >= max_keys {
            state.counters.retain(|_, counter| counter.expires_at > now);
            state.next_sweep = now + self.inner.sweep_interval;

            if state.counters.len() >= max_keys {
                let warn = state.capacity_warned_at.is_none_or(|at| {
                    now.saturating_duration_since(at) >= self.inner.sweep_interval
                });

                if warn {
                    state.capacity_warned_at = Some(now);

                    log::warn!(
                        "actix-limitation: in-memory store is at its {} key capacity; new \
                         keys are not being rate limited",
                        max_keys,
                    );
                }

                return (1, period);
            }
        }

        if start_new_window {
            state.counters.insert(
                key.to_owned(),
                Counter {
                    count: 0,
                    expires_at: now + period,
                },
            );
        }

        let counter = state
            .counters
            .get_mut(key)
            .expect("Counter should exist after initializing its window");

        counter.count += 1;

        (
            counter.count,
            counter.expires_at.saturating_duration_since(now),
        )
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        MemoryStore::new()
    }
}

// Omit counter keys and values from Debug output; show only the entry count.
impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counters = match self.inner.state.try_lock() {
            Ok(state) => Some(state.counters.len()),
            Err(TryLockError::Poisoned(err)) => Some(err.into_inner().counters.len()),
            Err(TryLockError::WouldBlock) => None,
        };

        let mut dbg = f.debug_struct("MemoryStore");

        dbg.field("max_keys", &self.inner.max_keys)
            .field("sweep_interval", &self.inner.sweep_interval);

        match counters {
            Some(len) => dbg.field("counters", &len),
            None => dbg.field("counters", &format_args!("<locked>")),
        };

        dbg.finish()
    }
}

/// Builder for [`MemoryStore`].
#[derive(Debug, Clone)]
pub struct MemoryStoreBuilder {
    max_keys: NonZeroUsize,
    sweep_interval: Duration,
}

impl MemoryStoreBuilder {
    /// Sets the maximum number of keys tracked at once.
    ///
    /// Because expired counters are removed lazily, the map can hold expired entries until the
    /// next sweep. When the store is full, a new key forces a sweep; if the store remains full,
    /// it fails open (the key is not tracked and the request passes) and logs a warning at most
    /// once per sweep interval.
    ///
    /// A `max` of `0` is clamped to `1`, never unbounded.
    ///
    /// Defaults to `usize::MAX`.
    #[must_use]
    pub fn max_keys(mut self, max: usize) -> Self {
        self.max_keys = NonZeroUsize::new(max).unwrap_or(NonZeroUsize::MIN);
        self
    }

    // Overrides the default sweep interval for tests.
    #[cfg(test)]
    fn sweep_interval(mut self, every: Duration) -> Self {
        self.sweep_interval = every;
        self
    }

    /// Builds the configured [`MemoryStore`] and logs a warning that counters are process-local.
    #[must_use]
    pub fn build(self) -> MemoryStore {
        log::warn!(
            "actix-limitation: using the in-memory store; rate limit counters are process-local \
             and are not shared between instances or preserved across restarts"
        );

        MemoryStore {
            inner: Arc::new(Inner {
                max_keys: self.max_keys,
                sweep_interval: self.sweep_interval,
                state: Mutex::new(State {
                    counters: HashMap::new(),
                    next_sweep: Instant::now() + self.sweep_interval,
                    capacity_warned_at: None,
                }),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    static_assertions::assert_impl_all!(MemoryStore: Send, Sync);
    static_assertions::assert_impl_all!(MemoryStoreBuilder: Send, Sync);

    /// Number of counters currently held, expired or not.
    fn tracked_keys(store: &MemoryStore) -> usize {
        store.inner.state.lock().unwrap().counters.len()
    }

    fn is_tracked(store: &MemoryStore, key: &str) -> bool {
        store.inner.state.lock().unwrap().counters.contains_key(key)
    }

    #[test]
    fn defaults() {
        let store = MemoryStore::new();
        assert_eq!(store.inner.max_keys, NonZeroUsize::MAX);
        assert_eq!(store.inner.sweep_interval, DEFAULT_SWEEP_INTERVAL);

        let store = MemoryStore::default();
        assert_eq!(store.inner.max_keys, NonZeroUsize::MAX);
    }

    #[test]
    fn debug_does_not_dump_counters() {
        let store = MemoryStore::new();
        store.track("some-very-distinctive-key", Duration::from_secs(60));

        let repr = format!("{store:?}");
        assert!(
            repr.starts_with("MemoryStore"),
            "Debug output should name the store: {repr}"
        );
        assert!(
            !repr.contains("some-very-distinctive-key"),
            "Debug output should omit counter keys: {repr}"
        );
        assert!(
            repr.contains("counters: 1"),
            "Debug output should include the counter count: {repr}"
        );
    }

    #[test]
    fn counts_increment_within_window() {
        let store = MemoryStore::new();
        let period = Duration::from_secs(60);

        for expected in 1..=5 {
            let (count, reset) = store.track("key", period);
            assert_eq!(count, expected);
            assert!(reset <= period);
        }

        assert_eq!(tracked_keys(&store), 1);
    }

    #[test]
    fn window_is_fixed_and_does_not_slide() {
        let store = MemoryStore::new();
        let period = Duration::from_millis(300);

        let (count, reset) = store.track("key", period);
        assert_eq!(count, 1);
        assert!(reset <= period);

        sleep(Duration::from_millis(80));

        let (count, reset) = store.track("key", period);
        assert_eq!(count, 2);
        assert!(
            reset <= Duration::from_millis(260),
            "Remaining time should decrease within a fixed window, got {reset:?} of {period:?}",
        );

        sleep(Duration::from_millis(300));

        let (count, reset) = store.track("key", period);
        assert_eq!(count, 1);
        assert!(reset > Duration::from_millis(260));
    }

    #[test]
    fn distinct_keys_are_independent() {
        let store = MemoryStore::new();
        let period = Duration::from_secs(60);

        assert_eq!(store.track("a", period).0, 1);
        assert_eq!(store.track("a", period).0, 2);
        assert_eq!(store.track("b", period).0, 1);
        assert_eq!(store.track("a", period).0, 3);
        assert_eq!(store.track("b", period).0, 2);

        assert_eq!(tracked_keys(&store), 2);
    }

    #[test]
    fn clones_share_counters() {
        let store = MemoryStore::new();
        let clone = store.clone();

        assert_eq!(store.track("key", Duration::from_secs(60)).0, 1);
        assert_eq!(clone.track("key", Duration::from_secs(60)).0, 2);
    }

    #[test]
    fn sweep_evicts_expired_entries_only() {
        let store = MemoryStore::builder()
            .sweep_interval(Duration::from_millis(50))
            .build();

        store.track("short", Duration::from_millis(30));
        store.track("long", Duration::from_secs(60));
        assert_eq!(tracked_keys(&store), 2);

        sleep(Duration::from_millis(80));

        store.track("trigger", Duration::from_secs(60));

        assert!(!is_tracked(&store, "short"));
        assert!(is_tracked(&store, "long"));
        assert!(is_tracked(&store, "trigger"));
        assert_eq!(tracked_keys(&store), 2);

        assert_eq!(store.track("long", Duration::from_secs(60)).0, 2);
    }

    #[test]
    fn full_store_fails_open() {
        let store = MemoryStore::builder()
            .max_keys(1)
            .sweep_interval(Duration::from_secs(60))
            .build();

        let period = Duration::from_secs(60);

        assert_eq!(store.track("a", period), (1, period));

        assert_eq!(store.track("b", period), (1, period));
        assert_eq!(store.track("b", period), (1, period));
        assert!(!is_tracked(&store, "b"));
        assert_eq!(tracked_keys(&store), 1);

        assert_eq!(store.track("a", period).0, 2);
    }

    #[test]
    fn full_store_sweeps_before_failing_open() {
        let store = MemoryStore::builder()
            .max_keys(1)
            .sweep_interval(Duration::from_secs(60))
            .build();

        store.track("a", Duration::from_millis(30));
        sleep(Duration::from_millis(50));

        assert_eq!(store.track("b", Duration::from_secs(60)).0, 1);
        assert!(is_tracked(&store, "b"));
        assert!(!is_tracked(&store, "a"));
    }

    #[test]
    fn max_keys_zero_is_treated_as_one() {
        let store = MemoryStore::builder().max_keys(0).build();
        assert_eq!(store.inner.max_keys, NonZeroUsize::MIN);

        assert_eq!(store.track("a", Duration::from_secs(60)).0, 1);
        assert_eq!(tracked_keys(&store), 1);
    }
}
