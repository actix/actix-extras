# Changes

## Unreleased - 2023-xx-xx

- Added optional scopes to the middleware enabling use of multiple Limiters by passing an `HashMap<Limiter>` to the Http server `app_data`

## 0.5.1

- No significant changes since `0.5.0`.

## 0.5.0

- Update `redis` dependency to `0.23`.
- Update `actix-session` dependency to `0.8`.

## 0.4.0

- Add `Builder::key_by` for setting a custom rate limit key function.
- Implement `Default` for `RateLimiter`.
- `RateLimiter` is marked `#[non_exhaustive]`; use `RateLimiter::default()` instead.
- In the middleware errors from the count function are matched and respond with `INTERNAL_SERVER_ERROR` if it's an unexpected error, instead of the default `TOO_MANY_REQUESTS`.
- Minimum supported Rust version (MSRV) is now 1.59 due to transitive `time` dependency.

## 0.3.0

- `Limiter::builder` now takes an `impl Into<String>`.
- Removed lifetime from `Builder`.
- Updated `actix-session` dependency to `0.7`.

## 0.2.0

- Update Actix Web dependency to v4 ecosystem.
- Update Tokio dependencies to v1 ecosystem.
- Rename `Limiter::{build => builder}()`.
- Rename `Builder::{finish => build}()`.
- Exceeding the rate limit now returns a 429 Too Many Requests response.

## 0.1.4

- Adopted into @actix org from <https://github.com/0xmad/actix-limitation>.
