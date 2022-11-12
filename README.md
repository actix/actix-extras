# actix-extras

> A collection of additional crates supporting [Actix Web].

[![CI](https://github.com/actix/actix-extras/actions/workflows/ci.yml/badge.svg)](https://github.com/actix/actix-extras/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/actix/actix-extras/branch/master/graph/badge.svg)](https://codecov.io/gh/actix/actix-extras)
[![Chat on Discord](https://img.shields.io/discord/771444961383153695?label=chat&logo=discord)](https://discord.gg/5Ux4QGChWc)
[![Dependency Status](https://deps.rs/repo/github/actix/actix-extras/status.svg)](https://deps.rs/repo/github/actix/actix-extras)

## Crates by @actix

| Crate                |                                                                                                                                                                                                                                                                |                                                                                 |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| [actix-cors]         | [![crates.io](https://img.shields.io/crates/v/actix-cors?label=latest)](https://crates.io/crates/actix-cors) [![dependency status](https://deps.rs/crate/actix-cors/0.6.1/status.svg)](https://deps.rs/crate/actix-cors/0.6.1)                                 | Cross-Origin Resource Sharing (CORS) controls.                                  |
| [actix-identity]     | [![crates.io](https://img.shields.io/crates/v/actix-identity?label=latest)](https://crates.io/crates/actix-identity) [![dependency status](https://deps.rs/crate/actix-identity/0.4.0/status.svg)](https://deps.rs/crate/actix-identity/0.4.0)                 | Identity management.                                                            |
| [actix-limitation]   | [![crates.io](https://img.shields.io/crates/v/actix-limitation?label=latest)](https://crates.io/crates/actix-limitation) [![dependency status](https://deps.rs/crate/actix-limitation/0.3.0/status.svg)](https://deps.rs/crate/actix-limitation/0.3.0)         | Rate-limiting using a fixed window counter for arbitrary keys, backed by Redis. |
| [actix-protobuf]     | [![crates.io](https://img.shields.io/crates/v/actix-protobuf?label=latest)](https://crates.io/crates/actix-protobuf) [![dependency status](https://deps.rs/crate/actix-protobuf/0.8.0/status.svg)](https://deps.rs/crate/actix-protobuf/0.8.0)                 | Protobuf payload extractor.                                                     |
| [actix-redis]        | [![crates.io](https://img.shields.io/crates/v/actix-redis?label=latest)](https://crates.io/crates/actix-redis) [![dependency status](https://deps.rs/crate/actix-redis/0.12.0/status.svg)](https://deps.rs/crate/actix-redis/0.12.0)                           | Actor-based Redis client.                                                       |
| [actix-session]      | [![crates.io](https://img.shields.io/crates/v/actix-session?label=latest)](https://crates.io/crates/actix-session) [![dependency status](https://deps.rs/crate/actix-session/0.7.1/status.svg)](https://deps.rs/crate/actix-session/0.7.1)                     | Session management.                                                             |
| [actix-settings]     | [![crates.io](https://img.shields.io/crates/v/actix-settings?label=latest)](https://crates.io/crates/actix-settings) [![dependency status](https://deps.rs/crate/actix-settings/0.6.0/status.svg)](https://deps.rs/crate/actix-settings/0.6.0)                 | Easily manage Actix Web's settings from a TOML file and environment variables.  |
| [actix-web-httpauth] | [![crates.io](https://img.shields.io/crates/v/actix-web-httpauth?label=latest)](https://crates.io/crates/actix-web-httpauth) [![dependency status](https://deps.rs/crate/actix-web-httpauth/0.8.0/status.svg)](https://deps.rs/crate/actix-web-httpauth/0.8.0) | HTTP authentication schemes.                                                    |

---

## Community Crates

These crates are provided by the community.

| Crate                      |                                                                                                                                                                                                                                                               |                                                                                                   |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| [actix-web-lab]            | [![crates.io](https://img.shields.io/crates/v/actix-web-lab?label=latest)][actix-web-lab] [![dependency status](https://deps.rs/crate/actix-web-lab/0.16.4/status.svg)](https://deps.rs/crate/actix-web-lab/0.16.4)                                           | Experimental extractors, middleware, and other extras for possible inclusion in Actix Web.        |
| [actix-multipart-extract]  | [![crates.io](https://img.shields.io/crates/v/actix-multipart-extract?label=latest)][actix-multipart-extract] [![dependency status](https://deps.rs/crate/actix-multipart-extract/0.1.4/status.svg)](https://deps.rs/crate/actix-multipart-extract/0.1.4)     | Better multipart form support for Actix Web.                                                      |
| [actix-form-data]          | [![crates.io](https://img.shields.io/crates/v/actix-form-data?label=latest)][actix-form-data] [![dependency status](https://deps.rs/crate/actix-form-data/0.6.2/status.svg)](https://deps.rs/crate/actix-form-data/0.6.2)                                     | Rate-limiting backed by form-data.                                                                |
| [actix-governor]           | [![crates.io](https://img.shields.io/crates/v/actix-governor?label=latest)][actix-governor] [![dependency status](https://deps.rs/crate/actix-governor/0.3.0/status.svg)](https://deps.rs/crate/actix-governor/0.3.0)                                         | Rate-limiting backed by governor.                                                                 |
| [actix-casbin]             | [![crates.io](https://img.shields.io/crates/v/actix-casbin?label=latest)][actix-casbin] [![dependency status](https://deps.rs/crate/actix-casbin/0.4.2/status.svg)](https://deps.rs/crate/actix-casbin/0.4.2)                                                 | Authorization library that supports access control models like ACL, RBAC & ABAC.                  |
| [actix-ip-filter]          | [![crates.io](https://img.shields.io/crates/v/actix-ip-filter?label=latest)][actix-ip-filter] [![dependency status](https://deps.rs/crate/actix-ip-filter/0.3.1/status.svg)](https://deps.rs/crate/actix-ip-filter/0.3.1)                                     | IP address filter. Supports glob patterns.                                                        |
| [actix-web-static-files]   | [![crates.io](https://img.shields.io/crates/v/actix-web-static-files?label=latest)][actix-web-static-files] [![dependency status](https://deps.rs/crate/actix-web-static-files/4.0.0/status.svg)](https://deps.rs/crate/actix-web-static-files/4.0.0)         | Static files as embedded resources.                                                               |
| [actix-web-grants]         | [![crates.io](https://img.shields.io/crates/v/actix-web-grants?label=latest)][actix-web-grants] [![dependency status](https://deps.rs/crate/actix-web-grants/3.0.1/status.svg)](https://deps.rs/crate/actix-web-grants/3.0.1)                                 | Extension for validating user authorities.                                                        |
| [aliri_actix]              | [![crates.io](https://img.shields.io/crates/v/aliri_actix?label=latest)][aliri_actix] [![dependency status](https://deps.rs/crate/aliri_actix/0.7.0/status.svg)](https://deps.rs/crate/aliri_actix/0.7.0)                                                     | Endpoint authorization and authentication using scoped OAuth2 JWT tokens.                         |
| [actix-web-flash-messages] | [![crates.io](https://img.shields.io/crates/v/actix-web-flash-messages?label=latest)][actix-web-flash-messages] [![dependency status](https://deps.rs/crate/actix-web-flash-messages/0.4.1/status.svg)](https://deps.rs/crate/actix-web-flash-messages/0.4.1) | Support for flash messages/one-time notifications in `actix-web`.                                 |
| [awmp]                     | [![crates.io](https://img.shields.io/crates/v/awmp?label=latest)][awmp] [![dependency status](https://deps.rs/crate/awmp/0.8.1/status.svg)](https://deps.rs/crate/awmp/0.8.1)                                                                                 | An easy to use wrapper around multipart fields for Actix Web.                                     |
| [tracing-actix-web]        | [![crates.io](https://img.shields.io/crates/v/tracing-actix-web?label=latest)][tracing-actix-web] [![dependency status](https://deps.rs/crate/tracing-actix-web/0.6.0/status.svg)](https://deps.rs/crate/tracing-actix-web/0.6.0)                             | A middleware to collect telemetry data from applications built on top of the actix-web framework. |
| [actix-ws]                 | [![crates.io](https://img.shields.io/crates/v/actix-ws?label=latest)][actix-ws] [![dependency status](https://deps.rs/crate/actix-ws/0.2.5/status.svg)](https://deps.rs/crate/actix-ws/0.2.5)                                                                 | Actor-less WebSockets for the Actix Runtime.                                                      |
| [actix-hash]               | [![crates.io](https://img.shields.io/crates/v/actix-hash?label=latest)][actix-hash] [![dependency status](https://deps.rs/crate/actix-hash/0.4.0/status.svg)](https://deps.rs/crate/actix-hash/0.4.0)                                                         | Hashing utilities for Actix Web.                                                                  |
| [actix-bincode](https://crates.io/crates/actix-bincode) | ![crates.io](https://img.shields.io/crates/v/actix-bincode?label=latest) [![dependency status](https://deps.rs/crate/actix-bincode/0.2.0/status.svg)](https://deps.rs/crate/actix-bincode/0.2.0) | Bincode payload extractor for Actix Web |

To add a crate to this list, submit a pull request.

<!-- REFERENCES -->

[actix]: https://github.com/actix/actix
[actix web]: https://github.com/actix/actix-web
[actix-extras]: https://github.com/actix/actix-extras
[actix-cors]: ./actix-cors
[actix-identity]: ./actix-identity
[actix-limitation]: ./actix-limitation
[actix-protobuf]: ./actix-protobuf
[actix-redis]: ./actix-redis
[actix-session]: ./actix-session
[actix-settings]: ./actix-settings
[actix-web-httpauth]: ./actix-web-httpauth
[actix-web-lab]: https://crates.io/crates/actix-web-lab
[actix-multipart-extract]: https://crates.io/crates/actix-multipart-extract
[actix-form-data]: https://crates.io/crates/actix-form-data
[actix-casbin]: https://crates.io/crates/actix-casbin
[actix-ip-filter]: https://crates.io/crates/actix-ip-filter
[actix-web-static-files]: https://crates.io/crates/actix-web-static-files
[actix-web-grants]: https://crates.io/crates/actix-web-grants
[actix-web-flash-messages]: https://crates.io/crates/actix-web-flash-messages
[actix-governor]: https://crates.io/crates/actix-governor
[aliri_actix]: https://crates.io/crates/aliri_actix
[awmp]: https://crates.io/crates/awmp
[tracing-actix-web]: https://crates.io/crates/tracing-actix-web
[actix-ws]: https://crates.io/crates/actix-ws
[actix-hash]: https://crates.io/crates/actix-hash
