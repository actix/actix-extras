# actix-settings

> Easily manage Actix Web's settings from a TOML file and environment variables.

<!-- prettier-ignore-start -->

[![crates.io](https://img.shields.io/crates/v/actix-settings?label=latest)](https://crates.io/crates/actix-settings)
[![Documentation](https://docs.rs/actix-settings/badge.svg?version=0.7.1)](https://docs.rs/actix-settings/0.7.1)
![Apache 2.0 or MIT licensed](https://img.shields.io/crates/l/actix-settings)
[![Dependency Status](https://deps.rs/crate/actix-settings/0.7.1/status.svg)](https://deps.rs/crate/actix-settings/0.7.1)

<!-- prettier-ignore-end -->

## Documentation & Resources

- [API Documentation](https://docs.rs/actix-settings)
- [Usage Example][usage]
- Minimum Supported Rust Version (MSRV): 1.57

### Custom Settings

There is a way to extend the available settings. This can be used to combine the settings provided by Actix Web and those provided by application server built using `actix`.

Have a look at [the usage example][usage] to see how.

## WIP

Configuration options for TLS set up are not yet implemented.

## Special Thanks

This crate was made possible by support from Accept B.V and [@jjpe].

[usage]: https://github.com/actix/actix-extras/blob/master/actix-settings/examples/actix.rs
[@jjpe]: https://github.com/jjpe
