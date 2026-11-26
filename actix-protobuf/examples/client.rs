//! Send and receive Protobuf messages with the Actix HTTP client.
//!
//! Run with: cargo run -p actix-protobuf --example client
//! The example starts a local server and stops it after checking the response.

use actix_protobuf::ProtoBuf;
use actix_web::{
    http::{header::CONTENT_TYPE, StatusCode},
    web, App, HttpServer,
};
use prost::Message;

#[derive(Clone, PartialEq, Message)]
struct Greeting {
    #[prost(string, tag = "1")]
    name: String,
}

async fn greet(message: ProtoBuf<Greeting>) -> ProtoBuf<Greeting> {
    ProtoBuf(Greeting {
        name: format!("Hello, {}!", message.name),
    })
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = HttpServer::new(|| App::new().route("/greet", web::post().to(greet)))
        .workers(1)
        .bind(("127.0.0.1", 0))?;
    let address = server.addrs()[0];
    let server = server.run();
    let handle = server.handle();
    actix_web::rt::spawn(server);

    let result = async {
        let message = Greeting {
            name: "Actix".into(),
        };

        // Encode the request with prost and set a content type accepted by ProtoBuf.
        let mut response = awc::Client::default()
            .post(format!("http://{address}/greet"))
            .insert_header((CONTENT_TYPE, "application/protobuf"))
            .send_body(message.encode_to_vec())
            .await?;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/protobuf"
        );

        // Read the response bytes before decoding them with the same message schema.
        let body = response.body().await?;
        let reply = Greeting::decode(body)?;

        assert_eq!(reply.name, "Hello, Actix!");
        println!("{}", reply.name);

        Ok(())
    }
    .await;

    handle.stop(true).await;
    result
}
