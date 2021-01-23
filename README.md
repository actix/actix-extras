# actix-extras

> A collection of additional crates supporting the [actix-web] and [actix] frameworks.

[![build status](https://github.com/actix/actix-extras/workflows/CI%20%28Linux%29/badge.svg?branch=master&event=push)](https://github.com/actix/actix-extras/actions)
[![Join the chat at https://gitter.im/actix/actix-web](https://badges.gitter.im/actix/actix-web.svg)](https://gitter.im/actix/actix-web?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)

## Crates by @actix

| Crate                |                                                                                                                                                                                                                                                                                                                                                                     |                                                                  |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| [actix-cors]         | [![crates.io](https://img.shields.io/crates/v/actix-cors?label=latest)](https://crates.io/crates/actix-cors) [![Documentation](https://docs.rs/actix-cors/badge.svg)](https://docs.rs/actix-cors) [![dependency status](https://deps.rs/crate/actix-cors/0.5.4/status.svg)](https://deps.rs/crate/actix-cors/0.5.4)                                                 | Cross-origin resource sharing (CORS) for actix-web applications. |
| [actix-identity]     | [![crates.io](https://img.shields.io/crates/v/actix-identity?label=latest)](https://crates.io/crates/actix-identity) [![Documentation](https://docs.rs/actix-identity/badge.svg)](https://docs.rs/actix-identity) [![dependency status](https://deps.rs/crate/actix-identity/0.3.1/status.svg)](https://deps.rs/crate/actix-identity/0.3.1)                         | Identity service for actix-web framework.                        |
| [actix-protobuf]     | [![crates.io](https://img.shields.io/crates/v/actix-protobuf?label=latest)](https://crates.io/crates/actix-protobuf) [![Documentation](https://docs.rs/actix-protobuf/badge.svg)](https://docs.rs/actix-protobuf) [![dependency status](https://deps.rs/crate/actix-protobuf/0.6.0/status.svg)](https://deps.rs/crate/actix-protobuf/0.6.0)                         | Protobuf support for actix-web framework.                        |
| [actix-redis]        | [![crates.io](https://img.shields.io/crates/v/actix-redis?label=latest)](https://crates.io/crates/actix-redis) [![Documentation](https://docs.rs/actix-redis/badge.svg)](https://docs.rs/actix-redis) [![dependency status](https://deps.rs/crate/actix-redis/0.9.1/status.svg)](https://deps.rs/crate/actix-redis/0.9.1)                                           | Redis integration for actix framework.                           |
| [actix-session]      | [![crates.io](https://img.shields.io/crates/v/actix-session?label=latest)](https://crates.io/crates/actix-session) [![Documentation](https://docs.rs/actix-session/badge.svg)](https://docs.rs/actix-session) [![dependency status](https://deps.rs/crate/actix-session/0.4.0/status.svg)](https://deps.rs/crate/actix-session/0.4.0)                               | Session for actix-web framework.                                 |
| [actix-web-httpauth] | [![crates.io](https://img.shields.io/crates/v/actix-web-httpauth?label=latest)](https://crates.io/crates/actix-web-httpauth) [![Documentation](https://docs.rs/actix-web-httpauth/badge.svg)](https://docs.rs/actix-web-httpauth) [![dependency status](https://deps.rs/crate/actix-web-httpauth/0.5.0/status.svg)](https://deps.rs/crate/actix-web-httpauth/0.5.0) | HTTP authentication schemes for actix-web.                       |

---

## Community Crates

These crates are provided by the community.

| Crate                    |                                                                                                                                                                                                                                                                                                                                                                                             |                                                                                     |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| [actix-limitation]       | [![crates.io](https://img.shields.io/crates/v/actix-limitation?label=latest)](https://crates.io/crates/actix-limitation) [![Documentation](https://docs.rs/actix-limitation/badge.svg)](https://docs.rs/actix-limitation) [![dependency status](https://deps.rs/crate/actix-limitation/0.1.4/status.svg)](https://deps.rs/crate/actix-limitation/0.1.4)                                     | Rate limiting using a fixed window counter for arbitrary keys, backed by Redis.      |
| [actix-casbin]           | [![crates.io](https://img.shields.io/crates/v/actix-casbin?label=latest)](https://crates.io/crates/actix-casbin) [![Documentation](https://docs.rs/actix-casbin/badge.svg)](https://docs.rs/actix-casbin) [![dependency status](https://deps.rs/crate/actix-casbin/0.4.2/status.svg)](https://deps.rs/crate/actix-casbin/0.4.2)                                                             | An authorization library that supports access control models like ACL, RBAC & ABAC. |
| [actix-web-static-files]  | [![crates.io](https://img.shields.io/crates/v/actix-web-static-files?label=latest)](https://crates.io/crates/actix-web-static-files) [![Documentation](https://docs.rs/actix-web-static-files/badge.svg)](https://docs.rs/actix-web-static-files) [![dependency status](https://deps.rs/crate/actix-web-static-files/3.0.1/status.svg)](https://deps.rs/crate/actix-web-static-files/3.0.1)       | Static files as embedded resources.                                                  |
| [actix-web-grants]       | [![crates.io](https://img.shields.io/crates/v/actix-web-grants?label=latest)](https://crates.io/crates/actix-web-grants) [![Documentation](https://docs.rs/actix-web-grants/badge.svg)](https://docs.rs/actix-web-grants) [![dependency status](https://deps.rs/repo/github/DDtKey/actix-web-grants/status.svg)](https://deps.rs/repo/github/DDtKey/actix-web-grants)                       | Extension for validating user authorities                                           |

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
[actix-limitation]: https://crates.io/crates/actix-limitation
[actix-casbin]: https://github.com/casbin-rs/actix-casbin
[actix-web-static-files]: https://github.com/kilork/actix-web-static-files
[actix-web-grants]: https://github.com/DDtKey/actix-web-grants
