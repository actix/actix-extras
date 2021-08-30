# Changes

## Unreleased - 2020-xx-xx
* Minimum supported Rust version (MSRV) is now 1.51.


## 0.10.0-beta.2 - 2020-06-27
* No notable changes.


## 0.10.0-beta.1 - 2020-04-02
* Update `actix-web` dependency to 4.0.0 beta.
* Minimum supported Rust version (MSRV) is now 1.46.0.


## 0.9.2 - 2020-03-21
* Implement `std::error::Error` for `Error` [#135]
* Allow the removal of `Max-Age` for session-only cookies. [#161]

[#135]: https://github.com/actix/actix-extras/pull/135
[#161]: https://github.com/actix/actix-extras/pull/161


## 0.9.1 - 2020-09-12
* Enforce minimum redis-async version of 0.6.3 to workaround breaking patch change.


## 0.9.0 - 2020-09-11
* Update `actix-web` dependency to 3.0.0.
* Minimize `futures` dependency.


## 0.9.0-alpha.2 - 2020-05-17
* Add `cookie_http_only` functionality to RedisSession builder, setting this
  to false allows JavaScript to access cookies. Defaults to true.
* Change type of parameter of ttl method to u32.
* Update `actix` to 0.10.0-alpha.3
* Update `tokio-util` to 0.3
* Minimum supported Rust version(MSRV) is now 1.40.0.


## 0.9.0-alpha.1 - 2020-03-28
* Update `actix` to 0.10.0-alpha.2
* Update `actix-session` to 0.4.0-alpha.1
* Update `actix-web` to 3.0.0-alpha.1
* Update `time` to 0.2.9


## 0.8.1 - 2020-02-18
* Move `env_logger` dependency to dev-dependencies and update to 0.7
* Update `actix_web` to 2.0.0 from 2.0.0-rc
* Move repository to actix-extras


## 0.8.0 - 2019-12-20
* Release


## 0.8.0-alpha.1 - 2019-12-16
* Migrate to actix 0.9


## 0.7.0 - 2019-09-25
* added cache_keygen functionality to RedisSession builder, enabling support for
  customizable cache key creation


## 0.6.1 - 2019-07-19
* remove ClonableService usage
* added comprehensive tests for session workflow


## 0.6.0 - 2019-07-08
* actix-web 1.0.0 compatibility
* Upgraded logic that evaluates session state, including new SessionStatus field,
  and introduced ``session.renew()`` and ``session.purge()`` functionality.
  Use ``renew()`` to cycle the session key at successful login.  ``renew()`` keeps a
  session's state while replacing the old cookie and session key with new ones.
  Use ``purge()`` at logout to invalidate the session cookie and remove the
  session's redis cache entry.


## 0.5.1 - 2018-08-02
* Use cookie 0.11


## 0.5.0 - 2018-07-21
* Session cookie configuration
* Actix/Actix-web 0.7 compatibility


## 0.4.0 - 2018-05-08
* Actix web 0.6 compatibility


## 0.3.0 - 2018-04-10
* Actix web 0.5 compatibility


## 0.2.0 - 2018-02-28
* Use resolver actor from actix
* Use actix web 0.5


## 0.1.0 - 2018-01-23
* First release
