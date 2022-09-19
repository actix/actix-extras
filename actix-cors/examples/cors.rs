use actix_cors::Cors;
use actix_web::{http::header, middleware::Logger, web, App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            // `permissive` is a wide-open development config
            // .wrap(Cors::permissive())
            .wrap(
                // default settings are overly restrictive to reduce chance of
                // misconfiguration leading to security concerns
                Cors::default()
                    // add specific origin to allowed origin list
                    .allowed_origin("http://project.local:8080")
                    // allow any port on localhost
                    .allowed_origin_fn(|origin, _req_head| {
                        origin.as_bytes().starts_with(b"http://localhost")

                        // manual alternative:
                        // unwrapping is acceptable on the origin header since this function is
                        // only called when it exists
                        // req_head
                        //     .headers()
                        //     .get(header::ORIGIN)
                        //     .unwrap()
                        //     .as_bytes()
                        //     .starts_with(b"http://localhost")
                    })
                    // set allowed methods list
                    .allowed_methods(vec!["GET", "POST"])
                    // set allowed request header list
                    .allowed_headers(&[header::AUTHORIZATION, header::ACCEPT])
                    // add header to allowed list
                    .allowed_header(header::CONTENT_TYPE)
                    // set list of headers that are safe to expose
                    .expose_headers(&[header::CONTENT_DISPOSITION])
                    // allow cURL/HTTPie from working without providing Origin headers
                    .block_on_origin_mismatch(false)
                    // set preflight cache TTL
                    .max_age(3600),
            )
            .wrap(Logger::default())
            .default_service(web::to(|| async { "Hello, cross-origin world!" }))
    })
    .workers(1)
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
