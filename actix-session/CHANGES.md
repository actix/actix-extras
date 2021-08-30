# Changes

## Unreleased - 2020-xx-xx
* Minimum supported Rust version (MSRV) is now 1.51.


## 0.5.0-beta.2 - 2020-06-27
* No notable changes.


## 0.5.0-beta.1 - 2020-04-02
* Add `Session::entries`. [#170]
* Rename `Session::{set => insert}` to match standard hash map naming. [#170]
* Return values from `Session::remove`. [#170]
* Add `Session::remove_as` deserializing variation. [#170]
* Simplify `Session::get_changes` now always returning iterator even when empty. [#170]
* Swap order of arguments on `Session::set_session`. [#170]
* Update `actix-web` dependency to 4.0.0 beta.
* Minimum supported Rust version (MSRV) is now 1.46.0.

[#170]: https://github.com/actix/actix-extras/pull/170


## 0.4.1 - 2020-03-21
* `Session::set_session` takes a `IntoIterator` instead of `Iterator`. [#105]
* Fix calls to `session.purge()` from paths other than the one specified in the cookie. [#129]

[#105]: https://github.com/actix/actix-extras/pull/105
[#129]: https://github.com/actix/actix-extras/pull/129


## 0.4.0 - 2020-09-11
* Update `actix-web` dependency to 3.0.0.
* Minimum supported Rust version (MSRV) is now 1.42.0.


## 0.4.0-alpha.1 - 2020-03-14
* Update the `time` dependency to 0.2.7
* Update the `actix-web` dependency to 3.0.0-alpha.1
* Long lasting auto-prolonged session [#1292]
* Minimize `futures` dependency

[#1292]: https://github.com/actix/actix-web/pull/1292


## 0.3.0 - 2019-12-20
* Release


## 0.3.0-alpha.4 - 2019-12-xx
* Allow access to sessions also from not mutable references to the request


## 0.3.0-alpha.3 - 2019-12-xx
* Add access to the session from RequestHead for use of session from guard methods
* Migrate to `std::future`
* Migrate to `actix-web` 2.0


## 0.2.0 - 2019-07-08
* Enhanced ``actix-session`` to facilitate state changes.  Use ``Session.renew()``
  at successful login to cycle a session (new key/cookie but keeps state).
  Use ``Session.purge()`` at logout to invalid a session cookie (and remove
  from redis cache, if applicable).


## 0.1.1 - 2019-06-03
* Fix optional cookie session support


## 0.1.0 - 2019-05-18
* Use actix-web 1.0.0-rc


## 0.1.0-beta.4 - 2019-05-12
* Use actix-web 1.0.0-beta.4


## 0.1.0-beta.2 - 2019-04-28
* Add helper trait `UserSession` which allows to get session for ServiceRequest and HttpRequest


## 0.1.0-beta.1 - 2019-04-20
* Update actix-web to beta.1
* `CookieSession::max_age()` accepts value in seconds


## 0.1.0-alpha.6 - 2019-04-14
* Update actix-web alpha.6


## 0.1.0-alpha.4 - 2019-04-08
* Update actix-web


## 0.1.0-alpha.3 - 2019-04-02
* Update actix-web


## 0.1.0-alpha.2 - 2019-03-29
* Update actix-web
* Use new feature name for secure cookies


## 0.1.0-alpha.1 - 2019-03-28
* Initial impl
