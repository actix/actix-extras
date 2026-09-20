# actix-web-httpauth

> HTTP authentication schemes for [Actix Web](https://actix.rs).

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-web-httpauth?label=latest)](https://crates.io/crates/actix-web-httpauth)
[![Documentation](https://docs.rs/actix-web-httpauth/badge.svg?version=0.8.3)](https://docs.rs/actix-web-httpauth/0.8.3)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-web-httpauth)
[![Dependency Status](https://deps.rs/crate/actix-web-httpauth/0.8.3/status.svg)](https://deps.rs/crate/actix-web-httpauth/0.8.3)

<!-- prettier-ignore-end -->

## Documentation & Resources

- [API Documentation](https://docs.rs/actix-web-httpauth/)
- Minimum Supported Rust Version (MSRV): 1.57

## Features

- Typed [Authorization] and [WWW-Authenticate] headers
- [Extractors] for authorization headers
- [Middleware] for easier authorization checking

All supported schemas can be used in both middleware and request handlers.

## Supported Schemes

- [HTTP Basic](https://tools.ietf.org/html/rfc7617)
- [OAuth Bearer](https://tools.ietf.org/html/rfc6750)

<!-- LINKS -->

[Authorization]: https://docs.rs/actix-web-httpauth/*/actix_web_httpauth/headers/authorization/index.html
[WWW-Authenticate]: https://docs.rs/actix-web-httpauth/*/actix_web_httpauth/headers/www_authenticate/index.html
[Extractors]: https://actix.rs/docs/extractors/
[Middleware]: https://docs.rs/actix-web-httpauth/*/actix_web_httpauth/middleware/index.html

## Client Example

Use `awc`, the Actix HTTP client, to send typed `Authorization` headers with `ClientRequest::insert_header`. The [client example](examples/client.rs) sends both Basic and Bearer credentials to a local server and checks the responses:

```sh
cargo run -p actix-web-httpauth --example client
```

The example starts and stops its own server. `awc` replaces the old `actix_web::client` module.
