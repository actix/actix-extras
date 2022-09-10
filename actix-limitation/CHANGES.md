# Changes

## Unreleased - 2022-xx-xx
- Add `Builder::key_by` for setting a custom rate limit key function.
- Implement `Default` for `RateLimiter`.
- `RateLimiter` is marked `#[non_exhaustive]`; use `RateLimiter::default()` instead.
- In the middleware errors from the count function are matched and respond with `INTERNAL_SERVER_ERROR` if it's an unexpected error, instead of the default `TOO_MANY_REQUESTS`.
- Minimum supported Rust version (MSRV) is now 1.59 due to transitive `time` dependency.


## 0.3.0 - 2022-07-11
- `Limiter::builder` now takes an `impl Into<String>`.
- Removed lifetime from `Builder`.
- Updated `actix-session` dependency to `0.7`.


## 0.2.0 - 2022-03-22
- Update Actix Web dependency to v4 ecosystem.
- Update Tokio dependencies to v1 ecosystem.
- Rename `Limiter::{build => builder}()`.
- Rename `Builder::{finish => build}()`.
- Exceeding the rate limit now returns a 429 Too Many Requests response.


## 0.1.4 - 2022-03-18
- Adopted into @actix org from <https://github.com/0xmad/actix-limitation>.
