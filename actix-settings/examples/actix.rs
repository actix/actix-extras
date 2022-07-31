use actix_settings::{ApplySettings as _, Mode, Settings};
use actix_web::{
    get,
    middleware::{Compress, Condition, Logger},
    web, App, HttpServer, Responder,
};

#[get("/")]
async fn index(settings: web::Data<Settings>) -> impl Responder {
    format!(
        r#"{{
  "mode": "{}",
  "hosts": ["{}"]
}}"#,
        match settings.actix.mode {
            Mode::Development => "development",
            Mode::Production => "production",
        },
        settings
            .actix
            .hosts
            .iter()
            .map(|addr| { format!("{}:{}", addr.host, addr.port) })
            .collect::<Vec<_>>()
            .join(", "),
    )
    .customize()
    .insert_header(("content-type", "application/json"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mut settings = Settings::parse_toml("./examples/Server.toml")
        .expect("Failed to parse `Settings` from Server.toml");

    // If the environment variable `$APPLICATION__HOSTS` is set,
    // have its value override the `settings.actix.hosts` setting:
    Settings::override_field_with_env_var(&mut settings.actix.hosts, "APPLICATION__HOSTS")?;

    init_logger(&settings);

    HttpServer::new({
        // clone settings into each worker thread
        let settings = settings.clone();

        move || {
            App::new()
                // Include this `.wrap()` call for compression settings to take effect:
                .wrap(Condition::new(
                    settings.actix.enable_compression,
                    Compress::default(),
                ))
                // make `Settings` available to handlers
                .wrap(Logger::default())
                .app_data(web::Data::new(settings.clone()))
                .service(index)
        }
    })
    // apply the `Settings` to Actix Web's `HttpServer`
    .apply_settings(&settings)
    .run()
    .await
}

/// Initialize the logging infrastructure
fn init_logger(settings: &Settings) {
    if !settings.actix.enable_log {
        return;
    }

    std::env::set_var(
        "RUST_LOG",
        match settings.actix.mode {
            Mode::Development => "actix_web=debug",
            Mode::Production => "actix_web=info",
        },
    );

    std::env::set_var("RUST_BACKTRACE", "1");

    env_logger::init();
}
