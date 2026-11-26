//! Rate limiting with the in-memory store — no Redis, no other service required.
//!
//! Run it:
//!
//! ```console
//! cargo run --example memory --features memory-store
//! curl -i localhost:8080     # 200 five times, then 429
//! ```
//!
//! The limit is 5 requests per 10 seconds per client IP, so you can also exhaust it by refreshing
//! <http://localhost:8080> in a browser and then watch it recover once the window rolls over.
//!
//! Requests are keyed on the client's socket address, so each client gets its own counter.
//!
//! See `examples/redis.rs` for the same app backed by Redis instead.

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use actix_limitation::{Limiter, MemoryStore, RateLimiter};
use actix_web::{dev::ServiceRequest, get, middleware, web, App, HttpServer, Responder};

const LIMIT: usize = 5;
const PERIOD: Duration = Duration::from_secs(10);

/// Counts requests that made it past the rate limiter, so the limit is visible in the response.
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
    // Initialise logging before the store is built, or its startup warning is dropped.
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Both backends can be compiled in at once, so a real app can pick one at startup from its
    // configuration — in-memory in development, Redis in production:
    //
    //     let mut builder = match env::var("REDIS_URL") {
    //         Ok(url) => Limiter::builder(url),
    //         Err(_) => Limiter::memory_builder(MemoryStore::new()),
    //     };
    let limiter = Limiter::memory_builder(MemoryStore::new())
        // Without an explicit key the default derives one from a session or cookie; a bare `curl`
        // has neither, and the middleware lets a `None` key through *unlimited*.
        //
        // `peer_addr` cannot be forged. `realip_remote_addr` trusts `X-Forwarded-For` instead, so
        // use it only behind a proxy you control — otherwise a client mints a fresh quota per
        // request.
        .key_by(|req: &ServiceRequest| req.connection_info().peer_addr().map(str::to_owned))
        .limit(LIMIT)
        .period(PERIOD)
        .build()
        .expect("limiter should build");

    // Build the limiter *once*, out here: the `HttpServer::new` closure runs per worker thread, so
    // building it inside would give each worker its own store and multiply the effective limit.
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
