use std::{
    collections::HashMap,
    fmt,
    num::NonZeroUsize,
    sync::{Arc, Mutex, PoisonError, TryLockError},
    time::{Duration, Instant},
};

/// Default interval between lazy sweeps of expired counters.
const DEFAULT_SWEEP_INTERVAL: Duration = Duration::from_secs(60);

/// A process-local, in-memory rate limit store.
///
/// **Counters live in this process only.** They are not shared with any other instance of your
/// application, and they are lost on restart. Run more than one instance — several pods, several
/// hosts, or a rolling deploy — and each keeps its own counts, so a client can be served by any of
/// them and the effective limit becomes `limit × instances`. Restarting an instance resets every
/// counter it held, letting clients that were over their limit start again immediately.
///
/// It is intended for local development, examples, and tests — none of which should need a Redis
/// server — and is a real implementation, not a stub: on a single instance it is correct, and safe
/// in production. Reach for [`Limiter::builder()`](crate::Limiter::builder) and Redis as soon as
/// there is more than one instance to share counts between.
///
/// A store is constructed once and cloned; cloning is cheap and all clones share one set of
/// counters. Constructing one logs a warning at `WARN` level, so an in-memory store shipped to
/// production by accident is visible in the logs.
///
/// The window is fixed, not sliding, matching the Redis backend: the first request for a key opens
/// a window of [`period`](crate::Builder::period), and later requests inside that window increment
/// the counter without extending it.
///
/// # Examples
///
/// ```no_run
/// use std::time::Duration;
/// use actix_web::{dev::ServiceRequest, web, App, HttpServer};
/// use actix_limitation::{Limiter, MemoryStore, RateLimiter};
///
/// #[actix_web::main]
/// async fn main() -> std::io::Result<()> {
///     // Build the limiter ONCE, here, outside the `HttpServer::new` closure, then clone the
///     // `web::Data` handle into each worker. The closure runs once per worker thread: building the
///     // limiter inside it would give every worker its own store, so nothing is shared and the
///     // effective limit is silently multiplied by the worker count.
///     let limiter = web::Data::new(
///         Limiter::memory_builder(MemoryStore::new())
///             .key_by(|req: &ServiceRequest| {
///                 req.connection_info().peer_addr().map(str::to_owned)
///             })
///             .limit(5)
///             .period(Duration::from_secs(10))
///             .build()
///             .unwrap(),
///     );
///
///     HttpServer::new(move || {
///         App::new()
///             .wrap(RateLimiter::default())
///             .app_data(limiter.clone())
///             .default_service(web::to(|| async { "Hello!" }))
///     })
///     .bind(("127.0.0.1", 8080))?
///     .run()
///     .await
/// }
/// ```
///
/// Configure the store with [`MemoryStore::builder()`]:
///
/// ```
/// use actix_limitation::MemoryStore;
///
/// let store = MemoryStore::builder().max_keys(10_000).build();
/// ```
///
/// # Differences From The Redis Backend
///
/// Rate limiting semantics are otherwise the same, but two behaviours differ, so that "works
/// locally, differs in production" does not come as a surprise:
///
/// - **Sub-second periods.** This store honours a [`period`](crate::Builder::period) of any
///   resolution. The Redis backend truncates it with [`Duration::as_secs`], so a period under one
///   second becomes `SET … EX 0`, which Redis rejects as an invalid expire time. Keep periods at
///   whole seconds if the same configuration has to run against both backends.
/// - **Behaviour at capacity.** With [`max_keys`](MemoryStoreBuilder::max_keys) set, this store
///   fails open: once it is full, a new key is not tracked at all and its requests pass unlimited.
///   Redis instead applies its own `maxmemory-policy`, which evicts existing counters — resetting
///   the windows of clients already being tracked — rather than letting new keys through.
///
/// # Limitations
///
/// Counters are process-local, as above: no sharing between instances, and no persistence across
/// restarts. Both are inherent, not bugs to be worked around.
///
/// Because the store is per-process, a [`Limiter`](crate::Limiter) built inside the
/// `HttpServer::new(…)` closure gives each worker thread its own counters. Build it once, outside,
/// and clone the [`web::Data`] handle in — see the example above.
///
/// Expired counters are removed lazily, by a sweep that runs on a write at most once a minute;
/// there is no background task. Memory use is therefore proportional to the number of distinct
/// keys seen within a sweep interval, and is
/// unbounded unless [`max_keys`](MemoryStoreBuilder::max_keys) is set — which trades that bound for
/// failing open, as above.
///
/// # Exclusivity
///
/// The `memory-store` and Redis backends may be compiled in at the same time, on purpose. **This
/// must not be "fixed" into a `compile_error!` on the combination.** It is not an oversight:
///
/// - A [`Limiter`](crate::Limiter) holds exactly one backend, fixed at construction by the
///   constructor used — [`Limiter::builder()`](crate::Limiter::builder) or
///   [`Limiter::memory_builder()`](crate::Limiter::memory_builder) — and [`Builder`](crate::Builder)
///   offers no way to change or add one afterwards. Using two backends at once is therefore not
///   merely discouraged, it is unrepresentable: no sequence of calls that compiles produces such a
///   limiter, so no runtime guard or feature-flag exclusion is needed.
/// - Mutually exclusive features would break the `--all-features` builds this crate relies on: the
///   docs.rs build (`all-features = true`), `cargo ci-test` and the `--all-features` clippy job.
/// - Cargo features are additive by contract. A `compile_error!` on the combination makes downstream
///   dependency graphs unbuildable through feature unification whenever two unrelated crates each
///   enable a different backend, with no recourse for the end user.
/// - It keeps backend selection at startup possible — memory locally, Redis in production, chosen
///   from configuration — which requires both to be compiled in.
///
/// [`web::Data`]: actix_web::web::Data
#[derive(Clone)]
pub struct MemoryStore {
    inner: Arc<Inner>,
}

struct Inner {
    max_keys: Option<NonZeroUsize>,
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
    /// Constructs an in-memory store with defaults: unbounded, swept every 60 seconds.
    ///
    /// Use [`MemoryStore::builder()`] to configure it.
    #[must_use]
    pub fn new() -> Self {
        MemoryStore::builder().build()
    }

    /// Constructs an in-memory store builder with defaults.
    #[must_use]
    pub fn builder() -> MemoryStoreBuilder {
        MemoryStoreBuilder {
            max_keys: None,
            sweep_interval: DEFAULT_SWEEP_INTERVAL,
        }
    }

    /// Tracks the given key in a period, returning the count and the time left in its window.
    ///
    /// Synchronous: there is no I/O to await.
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

        if counter.is_none() {
            if let Some(max_keys) = self.inner.max_keys {
                if state.counters.len() >= max_keys.get() {
                    state.counters.retain(|_, counter| counter.expires_at > now);
                    state.next_sweep = now + self.inner.sweep_interval;

                    if state.counters.len() >= max_keys.get() {
                        let warn = state.capacity_warned_at.is_none_or(|at| {
                            now.saturating_duration_since(at) >= self.inner.sweep_interval
                        });

                        if warn {
                            state.capacity_warned_at = Some(now);

                            log::warn!(
                                "actix-limitation: in-memory store is at its {} key capacity; new \
                                 keys are not being rate limited",
                                max_keys.get(),
                            );
                        }

                        return (1, period);
                    }
                }
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
            .expect("counter is either pre-existing or was just inserted");

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

/// Hand-written so that the counters themselves are never dumped; only how many there are.
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
    max_keys: Option<NonZeroUsize>,
    sweep_interval: Duration,
}

impl MemoryStoreBuilder {
    /// Sets the maximum number of keys tracked at once. Unbounded by default.
    ///
    /// Expired counters are only removed lazily, so the map can hold more keys than are live. When
    /// the store is full, a new key forces a sweep; if that frees nothing, the store **fails open**
    /// — the key is not tracked and the request passes — and warns at most once per sweep interval.
    ///
    /// A `max` of `0` is treated as `1`, never as unbounded.
    #[must_use]
    pub fn max_keys(mut self, max: usize) -> Self {
        self.max_keys = Some(NonZeroUsize::new(max).unwrap_or(NonZeroUsize::MIN));
        self
    }

    /// Shortens the otherwise fixed [`DEFAULT_SWEEP_INTERVAL`], so sweeping is observable in a
    /// test without sleeping for a minute.
    #[cfg(test)]
    fn sweep_interval(mut self, every: Duration) -> Self {
        self.sweep_interval = every;
        self
    }

    /// Constructs the configured [`MemoryStore`], warning once that its counters are process-local.
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
        assert!(store.inner.max_keys.is_none());
        assert_eq!(store.inner.sweep_interval, DEFAULT_SWEEP_INTERVAL);

        let store = MemoryStore::default();
        assert!(store.inner.max_keys.is_none());
    }

    #[test]
    fn store_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MemoryStore>();
        assert_send_sync::<MemoryStoreBuilder>();
    }

    #[test]
    fn debug_does_not_dump_counters() {
        let store = MemoryStore::new();
        store.track("some-very-distinctive-key", Duration::from_secs(60));

        let repr = format!("{store:?}");
        assert!(repr.starts_with("MemoryStore"), "{repr}");
        assert!(!repr.contains("some-very-distinctive-key"), "{repr}");
        assert!(repr.contains("counters: 1"), "{repr}");
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
            "window slid: {reset:?} left of a {period:?} window",
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
        assert_eq!(store.inner.max_keys, NonZeroUsize::new(1));

        assert_eq!(store.track("a", Duration::from_secs(60)).0, 1);
        assert_eq!(tracked_keys(&store), 1);
    }
}
