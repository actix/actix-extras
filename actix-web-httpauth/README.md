# actix-web-httpauth

[![crates.io](https://img.shields.io/crates/v/actix-web-httpauth)](https://crates.io/crates/actix-web-httpauth)
[![Documentation](https://docs.rs/actix-web-httpauth/badge.svg)](https://docs.rs/actix-web-httpauth)
[![Dependency Status](https://deps.rs/crate/actix-web-httpauth/0.4.2/status.svg)](https://deps.rs/crate/actix-web-httpauth/0.4.2)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-web-httpauth)
[![Join the chat at https://gitter.im/actix/actix](https://badges.gitter.im/actix/actix.svg)](https://gitter.im/actix/actix?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

> HTTP authentication schemes for [actix-web](https://github.com/actix/actix-web) framework.

Provides:
 * typed [Authorization] and [WWW-Authenticate] headers
 * [extractors] for an [Authorization] header
 * [middleware] for easier authorization checking

All supported schemas are actix [Extractors](https://docs.rs/actix-web/2.0.0/actix_web/trait.FromRequest.html),
and can be used both in the middlewares and request handlers.

## Supported schemes

 * [Basic](https://tools.ietf.org/html/rfc7617)
 * [Bearer](https://tools.ietf.org/html/rfc6750)
