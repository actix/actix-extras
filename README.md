# actix-extras

> A collection of additional crates supporting [Actix Web].

[![CI](https://github.com/actix/actix-extras/actions/workflows/ci.yml/badge.svg)](https://github.com/actix/actix-extras/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/actix/actix-extras/branch/master/graph/badge.svg)](https://codecov.io/gh/actix/actix-extras)
[![Chat on Discord](https://img.shields.io/discord/771444961383153695?label=chat&logo=discord)](https://discord.gg/5Ux4QGChWc)
[![Dependency Status](https://deps.rs/repo/github/actix/actix-extras/status.svg)](https://deps.rs/repo/github/actix/actix-extras)

## Crates by @actix

| Crate                |                                                                                                                                                                                                                                                           |                                                                                 |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| [actix-cors]         | [![crates.io](https://img.shields.io/crates/v/actix-cors?label=latest)](https://crates.io/crates/actix-cors) [![dependency status](https://deps.rs/crate/actix-cors/latest/status.svg)](https://deps.rs/crate/actix-cors)                                 | Cross-Origin Resource Sharing (CORS) controls.                                  |
| [actix-identity]     | [![crates.io](https://img.shields.io/crates/v/actix-identity?label=latest)](https://crates.io/crates/actix-identity) [![dependency status](https://deps.rs/crate/actix-identity/latest/status.svg)](https://deps.rs/crate/actix-identity)                 | Identity management.                                                            |
| [actix-limitation]   | [![crates.io](https://img.shields.io/crates/v/actix-limitation?label=latest)](https://crates.io/crates/actix-limitation) [![dependency status](https://deps.rs/crate/actix-limitation/latest/status.svg)](https://deps.rs/crate/actix-limitation)         | Rate-limiting using a fixed window counter for arbitrary keys, backed by Redis. |
| [actix-protobuf]     | [![crates.io](https://img.shields.io/crates/v/actix-protobuf?label=latest)](https://crates.io/crates/actix-protobuf) [![dependency status](https://deps.rs/crate/actix-protobuf/latest/status.svg)](https://deps.rs/crate/actix-protobuf)                 | Protobuf payload extractor.                                                     |
| [actix-redis]        | [![crates.io](https://img.shields.io/crates/v/actix-redis?label=latest)](https://crates.io/crates/actix-redis) [![dependency status](https://deps.rs/crate/actix-redis/latest/status.svg)](https://deps.rs/crate/actix-redis)                             | Actor-based Redis client.                                                       |
| [actix-session]      | [![crates.io](https://img.shields.io/crates/v/actix-session?label=latest)](https://crates.io/crates/actix-session) [![dependency status](https://deps.rs/crate/actix-session/latest/status.svg)](https://deps.rs/crate/actix-session)                     | Session management.                                                             |
| [actix-settings]     | [![crates.io](https://img.shields.io/crates/v/actix-settings?label=latest)](https://crates.io/crates/actix-settings) [![dependency status](https://deps.rs/crate/actix-settings/latest/status.svg)](https://deps.rs/crate/actix-settings)                 | Easily manage Actix Web's settings from a TOML file and environment variables.  |
| [actix-web-httpauth] | [![crates.io](https://img.shields.io/crates/v/actix-web-httpauth?label=latest)](https://crates.io/crates/actix-web-httpauth) [![dependency status](https://deps.rs/crate/actix-web-httpauth/latest/status.svg)](https://deps.rs/crate/actix-web-httpauth) | HTTP authentication schemes.                                                    |

---

## Community Crates

These crates are provided by the community.

| Crate                      |                                                                                                                                                                                                                                                          |                                                                                                   |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| [actix-web-lab]            | [![crates.io](https://img.shields.io/crates/v/actix-web-lab?label=latest)][actix-web-lab] [![dependency status](https://deps.rs/crate/actix-web-lab/latest/status.svg)](https://deps.rs/crate/actix-web-lab)                                             | Experimental extractors, middleware, and other extras for possible inclusion in Actix Web.        |
| [actix-multipart-extract]  | [![crates.io](https://img.shields.io/crates/v/actix-multipart-extract?label=latest)][actix-multipart-extract] [![dependency status](https://deps.rs/crate/actix-multipart-extract/latest/status.svg)](https://deps.rs/crate/actix-multipart-extract)     | Better multipart form support for Actix Web.                                                      |
| [actix-form-data]          | [![crates.io](https://img.shields.io/crates/v/actix-form-data?label=latest)][actix-form-data] [![dependency status](https://deps.rs/crate/actix-form-data/latest/status.svg)](https://deps.rs/crate/actix-form-data)                                 | Multipart form data from actix multipart streams                                                  |
| [actix-governor]           | [![crates.io](https://img.shields.io/crates/v/actix-governor?label=latest)][actix-governor] [![dependency status](https://deps.rs/crate/actix-governor/latest/status.svg)](https://deps.rs/crate/actix-governor)                                         | Rate-limiting backed by governor.                                                                 |
| [actix-casbin]             | [![crates.io](https://img.shields.io/crates/v/actix-casbin?label=latest)][actix-casbin] [![dependency status](https://deps.rs/crate/actix-casbin/latest/status.svg)](https://deps.rs/crate/actix-casbin)                                                 | Authorization library that supports access control models like ACL, RBAC & ABAC.                  |
| [actix-ip-filter]          | [![crates.io](https://img.shields.io/crates/v/actix-ip-filter?label=latest)][actix-ip-filter] [![dependency status](https://deps.rs/crate/actix-ip-filter/latest/status.svg)](https://deps.rs/crate/actix-ip-filter)                                     | IP address filter. Supports glob patterns.                                                        |
| [actix-web-static-files]   | [![crates.io](https://img.shields.io/crates/v/actix-web-static-files?label=latest)][actix-web-static-files] [![dependency status](https://deps.rs/crate/actix-web-static-files/latest/status.svg)](https://deps.rs/crate/actix-web-static-files)         | Static files as embedded resources.                                                               |
| [actix-web-grants]         | [![crates.io](https://img.shields.io/crates/v/actix-web-grants?label=latest)][actix-web-grants] [![dependency status](https://deps.rs/crate/actix-web-grants/latest/status.svg)](https://deps.rs/crate/actix-web-grants)                                 | Extension for validating user authorities.                                                        |
| [aliri_actix]              | [![crates.io](https://img.shields.io/crates/v/aliri_actix?label=latest)][aliri_actix] [![dependency status](https://deps.rs/crate/aliri_actix/latest/status.svg)](https://deps.rs/crate/aliri_actix)                                                     | Endpoint authorization and authentication using scoped OAuth2 JWT tokens.                         |
| [actix-web-flash-messages] | [![crates.io](https://img.shields.io/crates/v/actix-web-flash-messages?label=latest)][actix-web-flash-messages] [![dependency status](https://deps.rs/crate/actix-web-flash-messages/latest/status.svg)](https://deps.rs/crate/actix-web-flash-messages) | Support for flash messages/one-time notifications in `actix-web`.                                 |
| [awmp]                     | [![crates.io](https://img.shields.io/crates/v/awmp?label=latest)][awmp] [![dependency status](https://deps.rs/crate/awmp/latest/status.svg)](https://deps.rs/crate/awmp)                                                                                 | An easy to use wrapper around multipart fields for Actix Web.                                     |
| [tracing-actix-web]        | [![crates.io](https://img.shields.io/crates/v/tracing-actix-web?label=latest)][tracing-actix-web] [![dependency status](https://deps.rs/crate/tracing-actix-web/latest/status.svg)](https://deps.rs/crate/tracing-actix-web)                             | A middleware to collect telemetry data from applications built on top of the Actix Web framework. |
| [actix-ws]                 | [![crates.io](https://img.shields.io/crates/v/actix-ws?label=latest)][actix-ws] [![dependency status](https://deps.rs/crate/actix-ws/latest/status.svg)](https://deps.rs/crate/actix-ws)                                                                 | Actor-less WebSockets for the Actix Runtime.                                                      |
| [actix-hash]               | [![crates.io](https://img.shields.io/crates/v/actix-hash?label=latest)][actix-hash] [![dependency status](https://deps.rs/crate/actix-hash/latest/status.svg)](https://deps.rs/crate/actix-hash)                                                         | Hashing utilities for Actix Web.                                                                  |
| [actix-bincode]            | ![crates.io](https://img.shields.io/crates/v/actix-bincode?label=latest) [![dependency status](https://deps.rs/crate/actix-bincode/latest/status.svg)](https://deps.rs/crate/actix-bincode)                                                              | Bincode payload extractor for Actix Web                                                           |
| [sentinel-actix]           | ![crates.io](https://img.shields.io/crates/v/sentinel-actix?label=latest) [![dependency status](https://deps.rs/crate/sentinel-actix/latest/status.svg)](https://deps.rs/crate/sentinel-actix)                                                           | General and flexible protection for Actix Web                                                     |

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
[actix-bincode]: https://crates.io/crates/actix-bincode
[sentinel-actix]: https://crates.io/crates/sentinel-actix
