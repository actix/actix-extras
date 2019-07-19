use actix_web::{middleware, web, App, HttpServer};

use futures::future;

use actix_web_httpauth::middleware::HttpAuthentication;

fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let auth = HttpAuthentication::basic(|req, _credentials| future::ok(req));
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(auth)
            .service(web::resource("/").to(|| "Test\r\n"))
    })
    .bind("127.0.0.1:8080")?
    .workers(1)
    .run()
}
