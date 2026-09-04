//! Rate limiting with the Redis store — counters shared by every instance of the app.
//!
//! Start a Redis server, then run it:
//!
//! ```console
//! docker run --rm -p 6379:6379 redis:8
//! cargo run --example redis
//! curl -i localhost:8080     # 200 five times, then 429
//! ```
//!
//! Set `REDIS_URL` to point somewhere other than `redis://127.0.0.1:6379`.
//!
//! The limit is 5 requests per 10 seconds per client IP, so you can also exhaust it by refreshing
//! <http://localhost:8080> in a browser and then watch it recover once the window rolls over.
//! Because the counters live in Redis they survive a restart of this process, and a second copy of
//! the app on another port shares the same quota — run one and see.
//!
//! See `examples/memory.rs` for the same app with no server to run at all.

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
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS_URL.to_owned());
    log::info!("rate limiting via Redis at {redis_url}");

    // `build` only parses the URL; nothing connects to Redis until the first request is counted, so
    // a server that is down shows up as a 500 per request rather than as a failure to start.
    let limiter = Limiter::builder(redis_url)
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
