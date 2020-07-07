# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## unreleased (for alpha version)

* Minimum supported Rust version(MSRV) is now 1.40.0.

## [0.4.2] - 2020-07-08
  - Update the `base64` dependency to 0.12
  - AuthenticationError's status code is preserved when converting to a ResponseError
  - Minimize `futures` dependency
  - Fix panic on `AuthenticationMiddleware` [#69]

[#69]: https://github.com/actix/actix-web-httpauth/pull/69

## [0.4.1] - 2020-02-19
  - Move repository to actix-extras

## [0.4.0] - 2020-01-14

### Changed
  - Depends on `actix-web = "^2.0"`, `actix-service = "^1.0"`, and `futures = "^0.3"` version now ([#14])
  - Depends on `bytes = "^0.5"` and `base64 = "^0.11"` now

[#14]: https://github.com/actix/actix-web-httpauth/pull/14

## [0.3.2] - 2019-07-19

### Changed
  - Middleware accepts any `Fn` as a validator function instead of `FnMut` ([#11](https://github.com/actix/actix-web-httpauth/pull/11))

## [0.3.1] - 2019-06-09

### Fixed
  - Multiple calls to the middleware would result in panic

## [0.3.0] - 2019-06-05

### Changed
  - Crate edition was changed to `2018`, same as `actix-web`
  - Depends on `actix-web = "^1.0"` version now
  - `WWWAuthenticate` header struct was renamed into `WwwAuthenticate`
  - Challenges and extractor configs are now operating with `Cow<'static, str>` types instead of `String` types

## [0.2.0] - 2019-04-26

### Changed
  - `actix-web` dependency is used without default features now ([#6](https://github.com/actix/actix-web-httpauth/pull/6))
  - `base64` dependency version was bumped to `0.10`

## [0.1.0] - 2018-09-08

### Changed
  - Update to `actix-web = "0.7"` version

## [0.0.4] - 2018-07-01

### Fixed
  - Fix possible panic at `IntoHeaderValue` implementation for `headers::authorization::Basic`
  - Fix possible panic at `headers::www_authenticate::challenge::bearer::Bearer::to_bytes` call
