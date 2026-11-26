//! Rate limiting with the in-memory store (no Redis server required).
//!
//! Run:
//!
//! ```console
//! cargo run --example memory --features memory-store
//! curl -i localhost:8080 # 200 five times, then 429
//! ```
//!
//! Limits each client IP to 5 requests per 10 seconds.
//! See `examples/redis.rs` for the Redis-backed equivalent.

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use actix_limitation::{Limiter, MemoryStore, RateLimiter};
use actix_web::{dev::ServiceRequest, get, middleware, web, App, HttpServer, Responder};

const LIMIT: usize = 5;
const PERIOD: Duration = Duration::from_secs(10);

/// Counts requests that pass the rate limiter.
#[derive(Debug, Default)]
struct Served(AtomicUsize);

#[get("/")]
async fn index(served: web::Data<Served>) -> impl Responder {
    let n = served.0.fetch_add(1, Ordering::Relaxed) + 1;

    format!(
        "OK — request {n} served (limit: {LIMIT} requests per {}s per client IP)\n",
        PERIOD.as_secs(),
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging before building the store so its startup warning is emitted.
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Construct the limiter outside `HttpServer::new` so all worker threads share one store.
    let limiter = Limiter::memory_builder(MemoryStore::new())
        // `peer_addr` uses the socket address and cannot be spoofed via headers.
        // Use `realip_remote_addr` only behind a trusted proxy.
        .key_by(|req: &ServiceRequest| req.connection_info().peer_addr().map(str::to_owned))
        .limit(LIMIT)
        .period(PERIOD)
        .build()
        .expect("limiter should build");

    let limiter = web::Data::new(limiter);
    let served = web::Data::new(Served::default());

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(RateLimiter::default())
            .app_data(limiter.clone())
            .app_data(served.clone())
            .service(index)
    })
    .bind(("127.0.0.1", 8080))?
    .workers(2)
    .run()
    .await
}
