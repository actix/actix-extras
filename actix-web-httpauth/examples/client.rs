//! Send typed Authorization headers with awc, the Actix HTTP client.
//!
//! Run with: cargo run -p actix-web-httpauth --example client
//! The example starts a local server and stops it after checking both responses.

use actix_web::{http::StatusCode, web, App, HttpServer};
use actix_web_httpauth::{
    extractors::{basic::BasicAuth, bearer::BearerAuth},
    headers::authorization::{Authorization, Basic, Bearer},
};

async fn basic(auth: BasicAuth) -> &'static str {
    assert_eq!(auth.user_id(), "demo");
    assert_eq!(auth.password(), Some("password"));

    "Basic header received"
}

async fn bearer(auth: BearerAuth) -> &'static str {
    assert_eq!(auth.token(), "example-token");

    "Bearer header received"
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // These handlers check fixed example credentials. Use HTTPS and real credential
    // validation when sending authentication headers to a remote service.
    let server = HttpServer::new(|| {
        App::new()
            .route("/basic", web::get().to(basic))
            .route("/bearer", web::get().to(bearer))
    })
    .workers(1)
    .bind(("127.0.0.1", 0))?;

    let address = server.addrs()[0];
    let server = server.run();
    let handle = server.handle();

    actix_web::rt::spawn(server);

    let result = async {
        let client = awc::Client::default();
        let mut response = client
            .get(format!("http://{address}/basic"))
            .insert_header(Authorization::from(Basic::new("demo", Some("password"))))
            .send()
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body().await?, "Basic header received");

        let mut response = client
            .get(format!("http://{address}/bearer"))
            .insert_header(Authorization::from(Bearer::new("example-token")))
            .send()
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body().await?, "Bearer header received");
        println!("Basic and Bearer authorization requests succeeded.");

        Ok(())
    }
    .await;

    handle.stop(true).await;

    result
}
