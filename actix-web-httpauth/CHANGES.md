# Changes

## Unreleased - 2022-xx-xx
- Minimum supported Rust version (MSRV) is now 1.57 due to transitive `time` dependency.


## 0.6.0 - 2022-03-01
- Update `actix-web` dependency to `4`.


## 0.6.0-beta.8 - 2022-02-07
- Relax body type bounds on middleware impl. [#223]
- Update `actix-web` dependency to `4.0.0-rc.1`.

[#223]: https://github.com/actix/actix-extras/pull/223


## 0.6.0-beta.7 - 2021-12-29
- Minimum supported Rust version (MSRV) is now 1.54.


## 0.6.0-beta.6 - 2021-12-18
- Update `actix-web` dependency to `4.0.0.beta-15`. [#216]

[#216]: https://github.com/actix/actix-extras/pull/216


## 0.6.0-beta.5 - 2021-12-12
- Update `actix-web` dependency to `4.0.0.beta-14`. [#209]

[#209]: https://github.com/actix/actix-extras/pull/209


## 0.6.0-beta.4 - 2021-11-22
- impl `AuthExtractor` trait for `Option<T: AuthExtractor>` and `Result<T: AuthExtractor, T::Error>`. [#205]

[#205]: https://github.com/actix/actix-extras/pull/205


## 0.6.0-beta.3 - 2021-10-21
- Update `actix-web` dependency to v4.0.0-beta.10. [#203]
- Minimum supported Rust version (MSRV) is now 1.52.

[#203]: https://github.com/actix/actix-extras/pull/203


## 0.6.0-beta.2 - 2021-06-27
- No notable changes.


## 0.6.0-beta.1 - 2021-04-02
- Update `actix-web` dependency to 4.0.0 beta.
- Minimum supported Rust version (MSRV) is now 1.46.0.


## 0.5.1 - 2021-03-21
- Correct error handling when extracting auth details from request. [#128]

[#128]: https://github.com/actix/actix-extras/pull/128


## 0.5.0 - 2020-09-11
- Update `actix-web` dependency to 3.0.0.
- Minimum supported Rust version (MSRV) is now 1.42.0.


## 0.4.2 - 2020-07-08
- Update the `base64` dependency to 0.12
- AuthenticationError's status code is preserved when converting to a ResponseError
- Minimize `futures` dependency
- Fix panic on `AuthenticationMiddleware` [#69]

[#69]: https://github.com/actix/actix-web-httpauth/pull/69


## 0.4.1 - 2020-02-19
- Move repository to actix-extras


## 0.4.0 - 2020-01-14
- Depends on `actix-web = "^2.0"`, `actix-service = "^1.0"`, and `futures = "^0.3"` version now ([#14])
- Depends on `bytes = "^0.5"` and `base64 = "^0.11"` now

[#14]: https://github.com/actix/actix-web-httpauth/pull/14


## 0.3.2 - 2019-07-19
- Middleware accepts any `Fn` as a validator function instead of `FnMut` [#11]

[#11]: https://github.com/actix/actix-web-httpauth/pull/11


## 0.3.1 - 2019-06-09
- Multiple calls to the middleware would result in panic


## 0.3.0 - 2019-06-05
- Crate edition was changed to `2018`, same as `actix-web`
- Depends on `actix-web = "^1.0"` version now
- `WWWAuthenticate` header struct was renamed into `WwwAuthenticate`
- Challenges and extractor configs are now operating with `Cow<'static, str>` types instead of `String` types


## 0.2.0 - 2019-04-26
- `actix-web` dependency is used without default features now [#6]
- `base64` dependency version was bumped to `0.10`

[#6]: https://github.com/actix/actix-web-httpauth/pull/6


## 0.1.0 - 2018-09-08
- Update to `actix-web = "0.7"` version


## 0.0.4 - 2018-07-01
- Fix possible panic at `IntoHeaderValue` implementation for `headers::authorization::Basic`
- Fix possible panic at `headers::www_authenticate::challenge::bearer::Bearer::to_bytes` call
