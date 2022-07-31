# actix_settings

A Rust crate that allows for configuring `actix-web`'s [HttpServer](https://docs.rs/actix-web/4.1.0/actix_web/struct.HttpServer.html) instance through a `TOML` file.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
actix-settings = "0.6"
actix-web  = "4.1"
env_logger = "0.8"
```

### Basic usage

Import these items into your crate:

```rust
use actix_settings::{ApplySettings, AtResult, Settings};
use actix_web::{http::ContentEncoding, web};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let mut settings = Settings::parse_toml("Server.toml")
        .expect("Failed to parse `Settings` from Server.toml");

    // If the environment variable `$APPLICATION__HOSTS` is set,
    // have its value override the `settings.actix.hosts` setting:
    Settings::override_field_with_env_var(
        &mut settings.actix.hosts,
        "APPLICATION__HOSTS"
    )?;

    init_logger(&settings);

    HttpServer::new({
      let settings = settings.clone()
      
      move || {
          App::new()
              // Include this `.wrap()` call for compression settings to take effect:
              .wrap(Condition::new(
                  settings.actix.enable_compression,
                  Compress::default(),
              ))
              .wrap(Logger::default())

              // Make `Settings` available to handlers:
              .app_data(web::Data::new(settings.clone()))

              // Define routes as normal:
              .service(index)
          }
    })
    .apply_settings(&settings) // <- apply the `Settings` to actix's `HttpServer`
    .run()
    .await
}

/// Initialize the logging infrastructure
fn init_logger(settings: &Settings) {
    if !settings.actix.enable_log { return }
    std::env::set_var("RUST_LOG", match settings.actix.mode {
        Mode::Development => "actix_web=debug",
        Mode::Production  => "actix_web=info",
    });
    std::env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();
}
```

### Custom Settings

There is a way to extend the available settings. This can be used to combine
the settings provided by `actix-web` and those provided by application server
built using `actix`.

Have a look at the `override_extended_field_with_custom_type` test
in `src/lib.rs` to see how.

## WIP

The main feature that would be nice to have but currently is not implemented,
is `TLS`-support. If you're interested, please contact me or send a PR.

## Special Thanks

This crate was made possible by support from Accept B.V.
