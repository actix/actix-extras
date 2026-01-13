# Changes

## Unreleased

- Minimum supported Rust version (MSRV) is now 1.82.

## 0.9.0

- Update `toml` dependency to `0.9`.
- Minimum supported Rust version (MSRV) is now 1.82.

## 0.8.0

- Add `openssl` crate feature for TLS settings using OpenSSL.
- Add `ApplySettings::try_apply_settings()`.
- Implement TLS logic for `ApplySettings::try_apply_settings()`.
- Add `Tls::get_ssl_acceptor_builder()` function to build `openssl::ssl::SslAcceptorBuilder`.
- Deprecate `ApplySettings::apply_settings()`.
- Minimum supported Rust version (MSRV) is now 1.75.

## 0.7.1

- Fix doc examples.

## 0.7.0

- The `ApplySettings` trait now includes a type parameter, allowing multiple types to be implemented per configuration target.
- Implement `ApplySettings` for `ActixSettings`.
- `BasicSettings::from_default_template()` is now infallible.
- Rename `AtError => Error`.
- Remove `AtResult` type alias.
- Update `toml` dependency to `0.8`.
- Remove `ioe` dependency; `std::io::Error` is now used directly.
- Remove `Clone` implementation for `Error`.
- Implement `Display` for `Error`.
- Implement std's `Error` for `Error`.
- Minimum supported Rust version (MSRV) is now 1.68.

## 0.6.0

- Update Actix Web dependencies to v4 ecosystem.
- Rename `actix.ssl` settings object to `actix.tls`.
- `NoSettings` is now marked `#[non_exhaustive]`.

## 0.5.2

- Adopted into @actix org from <https://github.com/jjpe/actix-settings>.
