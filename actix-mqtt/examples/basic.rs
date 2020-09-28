use actix_mqtt::{Connect, ConnectAck, MqttServer, Publish};

#[derive(Clone)]
struct Session;

async fn connect<Io>(connect: Connect<Io>) -> Result<ConnectAck<Io, Session>, ()> {
    log::info!("new connection: {:?}", connect);
    Ok(connect.ack(Session, false))
}

async fn publish(publish: Publish<Session>) -> Result<(), ()> {
    log::info!(
        "incoming publish: {:?} -> {:?}",
        publish.id(),
        publish.topic()
    );
    Ok(())
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var(
        "RUST_LOG",
        "actix_server=trace,actix_mqtt=trace,basic=trace",
    );
    env_logger::init();

    actix_server::Server::build()
        .bind("mqtt", "127.0.0.1:1883", || {
            MqttServer::new(connect).finish(publish)
        })?
        .workers(1)
        .run()
        .await
}
