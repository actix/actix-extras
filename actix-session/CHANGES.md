# Changes

## Unreleased - 2021-xx-xx


## 0.7.0 - 2022-07-09
- Added `TtlExtensionPolicy` enum to support different strategies for extending the TTL attached to the session state. `TtlExtensionPolicy::OnEveryRequest` now allows for long-lived sessions that do not expire if the user remains active. [#233]
- `SessionLength` is now called `SessionLifecycle`. [#233]
- `SessionLength::Predetermined` is now called `SessionLifecycle::PersistentSession`. [#233]
- The fields for Both `SessionLength` variants have been extracted into separate types (`PersistentSession` and `BrowserSession`). All fields are now private, manipulated via methods, to allow adding more configuration parameters in the future in a non-breaking fashion. [#233]
- `SessionLength::Predetermined::max_session_length` is now called `PersistentSession::session_ttl`. [#233]
- `SessionLength::BrowserSession::state_ttl` is now called `BrowserSession::session_state_ttl`. [#233]
- `SessionMiddlewareBuilder::max_session_length` is now called `SessionMiddlewareBuilder::session_lifecycle`. [#233]
- The `SessionStore` trait requires the implementation of a new method, `SessionStore::update_ttl`. [#233]
- All types used to configure `SessionMiddleware` have been moved to the `config` sub-module [#233]
- Minimum supported Rust version (MSRV) is now 1.57 due to transitive `time` dependency.

[#233]: https://github.com/actix/actix-extras/pull/233


## 0.6.2 - 2022-03-25
- Implement `SessionExt` for `GuardContext`. [#234]
- `RedisSessionStore` will prevent connection timeouts from causing user-visible errors. [#235]
- Do not leak internal implementation details to callers when errors occur. [#236]

[#234]: https://github.com/actix/actix-extras/pull/234
[#236]: https://github.com/actix/actix-extras/pull/236
[#235]: https://github.com/actix/actix-extras/pull/235


## 0.6.1 - 2022-03-21
- No significant changes since `0.6.0`.


## 0.6.0 - 2022-03-15
### Added
- `SessionMiddleware`, a middleware to provide support for saving/updating/deleting session state against a pluggable storage backend (see `SessionStore` trait). [#212]
- `CookieSessionStore`, a cookie-based backend to store session state. [#212]
- `RedisActorSessionStore`, a Redis-based backend to store session state powered by `actix-redis`. [#212]
- `RedisSessionStore`, a Redis-based backend to store session state powered by `redis-rs`. [#212]
- Add TLS support for Redis via `RedisSessionStore`. [#212]
- Implement `SessionExt` for `ServiceResponse`. [#212]

### Changed
- Rename `UserSession` to `SessionExt`. [#212]

### Removed
- `CookieSession`; replaced with `CookieSessionStore`, a storage backend for `SessionMiddleware`. [#212]
- `Session::set_session`; use `Session::insert` to modify the session state. [#212]

[#212]: https://github.com/actix/actix-extras/pull/212


## 0.5.0 - 2022-03-01
- Update `actix-web` dependency to `4`.


## 0.5.0-beta.8 - 2022-02-07
- Update `actix-web` dependency to `4.0.0-rc.1`.


## 0.5.0-beta.7 - 2021-12-29
- Update `actix-web` dependency to `4.0.0.beta-18`. [#218]
- Minimum supported Rust version (MSRV) is now 1.54.

[#218]: https://github.com/actix/actix-extras/pull/218


## 0.5.0-beta.6 - 2021-12-18
- Update `actix-web` dependency to `4.0.0.beta-15`. [#216]

[#216]: https://github.com/actix/actix-extras/pull/216


## 0.5.0-beta.5 - 2021-12-12
- Update `actix-web` dependency to `4.0.0.beta-14`. [#209]
- Remove `UserSession` implementation for `RequestHead`. [#209]
- A session will be created in the storage backend if and only if there is some data inside the session state. This reduces the performance impact of `SessionMiddleware` on routes that do not leverage sessions. [#207]

[#207]: https://github.com/actix/actix-extras/pull/207
[#209]: https://github.com/actix/actix-extras/pull/209


## 0.5.0-beta.4 - 2021-11-22
- No significant changes since `0.5.0-beta.3`.


## 0.5.0-beta.3 - 2021-10-21
- Impl `Clone` for `CookieSession`. [#201]
- Update `actix-web` dependency to v4.0.0-beta.10. [#203]
- Minimum supported Rust version (MSRV) is now 1.52.

[#201]: https://github.com/actix/actix-extras/pull/201
[#203]: https://github.com/actix/actix-extras/pull/203


## 0.5.0-beta.2 - 2021-06-27
- No notable changes.


## 0.5.0-beta.1 - 2021-04-02
- Add `Session::entries`. [#170]
- Rename `Session::{set => insert}` to match standard hash map naming. [#170]
- Return values from `Session::remove`. [#170]
- Add `Session::remove_as` deserializing variation. [#170]
- Simplify `Session::get_changes` now always returning iterator even when empty. [#170]
- Swap order of arguments on `Session::set_session`. [#170]
- Update `actix-web` dependency to 4.0.0 beta.
- Minimum supported Rust version (MSRV) is now 1.46.0.

[#170]: https://github.com/actix/actix-extras/pull/170


## 0.4.1 - 2021-03-21
- `Session::set_session` takes a `IntoIterator` instead of `Iterator`. [#105]
- Fix calls to `session.purge()` from paths other than the one specified in the cookie. [#129]

[#105]: https://github.com/actix/actix-extras/pull/105
[#129]: https://github.com/actix/actix-extras/pull/129


## 0.4.0 - 2020-09-11
- Update `actix-web` dependency to 3.0.0.
- Minimum supported Rust version (MSRV) is now 1.42.0.


## 0.4.0-alpha.1 - 2020-03-14
- Update the `time` dependency to 0.2.7
- Update the `actix-web` dependency to 3.0.0-alpha.1
- Long lasting auto-prolonged session [#1292]
- Minimize `futures` dependency

[#1292]: https://github.com/actix/actix-web/pull/1292


## 0.3.0 - 2019-12-20
- Release


## 0.3.0-alpha.4 - 2019-12-xx
- Allow access to sessions also from not mutable references to the request


## 0.3.0-alpha.3 - 2019-12-xx
- Add access to the session from RequestHead for use of session from guard methods
- Migrate to `std::future`
- Migrate to `actix-web` 2.0


## 0.2.0 - 2019-07-08
- Enhanced ``actix-session`` to facilitate state changes.  Use ``Session.renew()``
  at successful login to cycle a session (new key/cookie but keeps state).
  Use ``Session.purge()`` at logout to invalid a session cookie (and remove
  from redis cache, if applicable).


## 0.1.1 - 2019-06-03
- Fix optional cookie session support


## 0.1.0 - 2019-05-18
- Use actix-web 1.0.0-rc


## 0.1.0-beta.4 - 2019-05-12
- Use actix-web 1.0.0-beta.4


## 0.1.0-beta.2 - 2019-04-28
- Add helper trait `UserSession` which allows to get session for ServiceRequest and HttpRequest


## 0.1.0-beta.1 - 2019-04-20
- Update actix-web to beta.1
- `CookieSession::max_age()` accepts value in seconds


## 0.1.0-alpha.6 - 2019-04-14
- Update actix-web alpha.6


## 0.1.0-alpha.4 - 2019-04-08
- Update actix-web


## 0.1.0-alpha.3 - 2019-04-02
- Update actix-web


## 0.1.0-alpha.2 - 2019-03-29
- Update actix-web
- Use new feature name for secure cookies


## 0.1.0-alpha.1 - 2019-03-28
- Initial impl
