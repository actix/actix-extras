# actix-extras

> A collection of additional crates supporting the [actix-web] and [actix] frameworks.

[![CI](https://github.com/actix/actix-extras/actions/workflows/ci.yml/badge.svg)](https://github.com/actix/actix-extras/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/actix/actix-extras/branch/master/graph/badge.svg)](https://codecov.io/gh/actix/actix-extras)
[![Chat on Discord](https://img.shields.io/discord/771444961383153695?label=chat&logo=discord)](https://discord.gg/5Ux4QGChWc)
[![Dependency Status](https://deps.rs/repo/github/actix/actix-extras/status.svg)](https://deps.rs/repo/github/actix/actix-extras)

## Crates by @actix

| Crate                |                                                                                                                                                                                                                                                                |                                                                  |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| [actix-cors]         | [![crates.io](https://img.shields.io/crates/v/actix-cors?label=latest)](https://crates.io/crates/actix-cors) [![dependency status](https://deps.rs/crate/actix-cors/0.6.1/status.svg)](https://deps.rs/crate/actix-cors/0.6.1)                                 | Cross-origin resource sharing (CORS) for actix-web applications. |
| [actix-identity]     | [![crates.io](https://img.shields.io/crates/v/actix-identity?label=latest)](https://crates.io/crates/actix-identity) [![dependency status](https://deps.rs/crate/actix-identity/0.4.0/status.svg)](https://deps.rs/crate/actix-identity/0.4.0)                 | Identity service for actix-web framework.                        |
| [actix-protobuf]     | [![crates.io](https://img.shields.io/crates/v/actix-protobuf?label=latest)](https://crates.io/crates/actix-protobuf) [![dependency status](https://deps.rs/crate/actix-protobuf/0.7.0/status.svg)](https://deps.rs/crate/actix-protobuf/0.7.0)                 | Protobuf support for actix-web framework.                        |
| [actix-redis]        | [![crates.io](https://img.shields.io/crates/v/actix-redis?label=latest)](https://crates.io/crates/actix-redis) [![dependency status](https://deps.rs/crate/actix-redis/0.11.0/status.svg)](https://deps.rs/crate/actix-redis/0.11.0)                           | Redis integration for actix framework.                           |
| [actix-session]      | [![crates.io](https://img.shields.io/crates/v/actix-session?label=latest)](https://crates.io/crates/actix-session) [![dependency status](https://deps.rs/crate/actix-session/0.6.0/status.svg)](https://deps.rs/crate/actix-session/0.6.0)                     | Session for actix-web framework.                                 |
| [actix-web-httpauth] | [![crates.io](https://img.shields.io/crates/v/actix-web-httpauth?label=latest)](https://crates.io/crates/actix-web-httpauth) [![dependency status](https://deps.rs/crate/actix-web-httpauth/0.6.0/status.svg)](https://deps.rs/crate/actix-web-httpauth/0.6.0) | HTTP authentication schemes for actix-web.                       |

---

## Community Crates

These crates are provided by the community.

| Crate                      |                                                                                                                                                                                                                                                                                        |                                                                                                   |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| [actix-form-data]          | [![crates.io](https://img.shields.io/crates/v/actix-form-data?label=latest)](https://crates.io/crates/actix-form-data) [![dependency status](https://deps.rs/crate/actix-form-data/0.6.2/status.svg)](https://deps.rs/crate/actix-form-data/0.6.2)                                     | Rate-limiting backed by form-data.                                                                |
| [actix-governor]           | [![crates.io](https://img.shields.io/crates/v/actix-governor?label=latest)](https://crates.io/crates/actix-governor) [![dependency status](https://deps.rs/crate/actix-governor/0.3.0/status.svg)](https://deps.rs/crate/actix-governor/0.3.0)                                         | Rate-limiting backed by governor.                                                                 |
| [actix-limitation]         | [![crates.io](https://img.shields.io/crates/v/actix-limitation?label=latest)](https://crates.io/crates/actix-limitation) [![dependency status](https://deps.rs/crate/actix-limitation/0.1.4/status.svg)](https://deps.rs/crate/actix-limitation/0.1.4)                                 | Rate-limiting using a fixed window counter for arbitrary keys, backed by Redis.                   |
| [actix-casbin]             | [![crates.io](https://img.shields.io/crates/v/actix-casbin?label=latest)](https://crates.io/crates/actix-casbin) [![dependency status](https://deps.rs/crate/actix-casbin/0.4.2/status.svg)](https://deps.rs/crate/actix-casbin/0.4.2)                                                 | Authorization library that supports access control models like ACL, RBAC & ABAC.                  |
| [actix-ip-filter]          | [![crates.io](https://img.shields.io/crates/v/actix-ip-filter?label=latest)](https://crates.io/crates/actix-ip-filter) [![dependency status](https://deps.rs/crate/actix-ip-filter/0.3.1/status.svg)](https://deps.rs/crate/actix-ip-filter/0.3.1)                                     | IP address filter. Supports glob patterns.                                                        |
| [actix-web-static-files]   | [![crates.io](https://img.shields.io/crates/v/actix-web-static-files?label=latest)](https://crates.io/crates/actix-web-static-files) [![dependency status](https://deps.rs/crate/actix-web-static-files/4.0.0/status.svg)](https://deps.rs/crate/actix-web-static-files/4.0.0)         | Static files as embedded resources.                                                               |
| [actix-web-grants]         | [![crates.io](https://img.shields.io/crates/v/actix-web-grants?label=latest)](https://crates.io/crates/actix-web-grants) [![dependency status](https://deps.rs/crate/actix-web-grants/3.0.0-beta.6/status.svg)](https://deps.rs/crate/actix-web-grants/3.0.0-beta.6)                   | Extension for validating user authorities.                                                        |
| [aliri_actix]              | [![crates.io](https://img.shields.io/crates/v/aliri_actix?label=latest)](https://crates.io/crates/aliri_actix) [![dependency status](https://deps.rs/crate/aliri_actix/0.5.1/status.svg)](https://deps.rs/crate/aliri_actix/0.5.1)                                                     | Endpoint authorization and authentication using scoped OAuth2 JWT tokens.                         |
| [actix-web-flash-messages] | [![crates.io](https://img.shields.io/crates/v/actix-web-flash-messages?label=latest)](https://crates.io/crates/actix-web-flash-messages) [![dependency status](https://deps.rs/crate/actix-web-flash-messages/0.3.2/status.svg)](https://deps.rs/crate/actix-web-flash-messages/0.3.2) | Support for flash messages/one-time notifications in `actix-web`.                                 |
| [awmp]                     | [![crates.io](https://img.shields.io/crates/v/awmp?label=latest)](https://crates.io/crates/awmp) [![dependency status](https://deps.rs/crate/awmp/0.8.1/status.svg)](https://deps.rs/crate/awmp/0.8.1)                                                                                 | An easy to use wrapper around multipart fields for Actix Web.                                     |
| [tracing-actix-web]        | [![crates.io](https://img.shields.io/crates/v/tracing-actix-web?label=latest)](https://crates.io/crates/tracing-actix-web) [![dependency status](https://deps.rs/crate/tracing-actix-web/0.5.1/status.svg)](https://deps.rs/crate/tracing-actix-web/0.5.1)                             | A middleware to collect telemetry data from applications built on top of the actix-web framework. |
| [actix-ws]                 | [![crates.io](https://img.shields.io/crates/v/actix-ws?label=latest)](https://crates.io/crates/actix-ws) [![dependency status](https://deps.rs/crate/actix-ws/0.2.5/status.svg)](https://deps.rs/crate/actix-ws/0.2.5)                                                                 | A middleware to collect telemetry data from applications built on top of the actix-web framework. |

To add a crate to this list, submit a pull request.

<!-- REFERENCES -->

[actix]: https://github.com/actix/actix
[actix-web]: https://github.com/actix/actix-web
[actix-extras]: https://github.com/actix/actix-extras
[actix-cors]: actix-cors
[actix-identity]: actix-identity
[actix-protobuf]: actix-protobuf
[actix-redis]: actix-redis
[actix-session]: actix-session
[actix-web-httpauth]: actix-web-httpauth
[actix-form-data]: https://git.asonix.dog/asonix/actix-form-data
[actix-limitation]: https://github.com/0xmad/actix-limitation
[actix-casbin]: https://github.com/casbin-rs/actix-casbin
[actix-ip-filter]: https://github.com/jhen0409/actix-ip-filter
[actix-web-static-files]: https://github.com/kilork/actix-web-static-files
[actix-web-grants]: https://github.com/DDtKey/actix-web-grants
[actix-web-flash-messages]: https://github.com/LukeMathWalker/actix-web-flash-messages
[actix-governor]: https://github.com/AaronErhardt/actix-governor
[aliri_actix]: https://github.com/neoeinstein/aliri
[awmp]: https://github.com/kardeiz/awmp
[tracing-actix-web]: https://github.com/LukeMathWalker/tracing-actix-web
[actix-ws]: https://git.asonix.dog/asonix/actix-actorless-websockets
