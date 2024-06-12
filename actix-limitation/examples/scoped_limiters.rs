use std::{collections::HashMap, time::Duration};

use actix_limitation::{Limiter, RateLimiter};
use actix_web::{dev::ServiceRequest, get, put, web, App, HttpServer, Responder};
use redis::Client;

#[get("/")]
async fn index() -> impl Responder {
    "index"
}

#[put("/sms")]
async fn send_sms() -> impl Responder {
    "sending an expensive sms"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    // Create an Hashmap to store the multiples [Limiter](Limiter)
    let mut limiters = HashMap::new();

    // Create and connect a redis Client.
    let redis_client = Client::open("redis://127.0.0.1/").expect("creation of the redis client");

    // Create a default limiter
    let default_limiter = Limiter::builder_with_redis_client(redis_client.clone())
        // specifying with key_by that we take the user IP address as a identifier.
        .key_by(|req: &ServiceRequest| {
            req.connection_info()
                .realip_remote_addr()
                .map(|ip| ip.to_string())
        })
        // Allowing a maximum of 30 requests per minute
        .limit(30)
        .period(Duration::from_secs(60))
        .build()
        .unwrap();
    limiters.insert("default", default_limiter);

    let scope_limiter = Limiter::builder_with_redis_client(redis_client)
        .key_by(|req: &ServiceRequest| {
            req.connection_info()
                .realip_remote_addr()
                // ⚠️ we prepend "scoped" to the key in order to isolate this count from the default count
                //
                // If we were using the same key, a request to this route would always return too many requests
                // in this context because the default limiter at the root would be reached first and would count 1 before we check for this.
                // To mitigate this issue you could also specify a different namespace with the redis_client passed as parameter: `redis://127.0.0.1/2`
                .map(|ip| format!("scoped-{}", ip))
        })
        // Allowing only 1 request per minute
        .limit(1)
        .period(Duration::from_secs(60))
        .build()
        .unwrap();
    limiters.insert("scoped", scope_limiter);

    // Passing this limiters as app_data so it can be accessed by the middleware.
    let limiters = web::Data::new(limiters);
    HttpServer::new(move || {
        App::new()
            // Using the default limiter for all the routes
            // ⚠️ This limiter will count and apply the limits before the one in "/scoped"
            .wrap(RateLimiter::scoped("default"))
            .app_data(limiters.clone())
            .service(
                web::scope("/scoped")
                    // Wrapping only for this scope the scoped limiter
                    .wrap(RateLimiter::scoped("scoped"))
                    // This route will only be available 1 time every minutes
                    // Note: the root limiter default will also limit this route
                    .service(send_sms),
            )
            // This route is only limited by the default limiter
            .service(index)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
