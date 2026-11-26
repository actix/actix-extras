//! Rate limiting with the Redis store.
//!
//! Start Redis, then run:
//!
//! ```console
//! docker run --rm -p 6379:6379 redis:8
//! cargo run --example redis
//! curl -i localhost:8080 # 200 five times, then 429
//! ```
//!
//! Set `REDIS_URL` to override `redis://127.0.0.1:6379`.
//! Limits each client IP to 5 requests per 10 seconds, shared across instances and restarts.
//! See `examples/memory.rs` for the in-memory equivalent.

use std::{
    env,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use actix_limitation::{Limiter, RateLimiter};
use actix_web::{dev::ServiceRequest, get, middleware, web, App, HttpServer, Responder};

const LIMIT: usize = 5;
const PERIOD: Duration = Duration::from_secs(10);
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";

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
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS_URL.to_owned());
    log::info!("rate limiting via Redis at {redis_url}");

    // `build` parses the URL without connecting; Redis connection errors occur on request handling.
    let limiter = Limiter::builder(redis_url)
        // `peer_addr` uses the socket address and cannot be spoofed via headers.
        // Use `realip_remote_addr` only behind a trusted proxy.
        .key_by(|req: &ServiceRequest| req.connection_info().peer_addr().map(str::to_owned))
        .limit(LIMIT)
        .period(PERIOD)
        .build()
        .expect("limiter should build");

    // Share one `Limiter` handle across all worker threads.
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
