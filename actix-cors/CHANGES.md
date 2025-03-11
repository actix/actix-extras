# Changes

## Unreleased

## 0.7.1

- Implement `PartialEq` for `Cors` allowing for better testing.

## 0.7.0

- `Cors` is now marked `#[must_use]`.
- Default for `Cors::block_on_origin_mismatch` is now false.
- Minimum supported Rust version (MSRV) is now 1.75.

## 0.6.5

- Fix `Vary` header when Private Network Access is enabled.
- Minimum supported Rust version (MSRV) is now 1.68.

## 0.6.4

- Add `Cors::allow_private_network_access()` behind an unstable flag (`draft-private-network-access`).

## 0.6.3

- Add `Cors::block_on_origin_mismatch()` option for controlling if requests are pre-emptively rejected.
- Minimum supported Rust version (MSRV) is now 1.59 due to transitive `time` dependency.

## 0.6.2

- Fix `expose_any_header` to return list of response headers.
- Minimum supported Rust version (MSRV) is now 1.57 due to transitive `time` dependency.

## 0.6.1

- Do not consider requests without a `Access-Control-Request-Method` as preflight.

## 0.6.0

- Update `actix-web` dependency to 4.0.

<details>
<summary>0.6.0 pre-releases</summary>

## 0.6.0-beta.10

- Ensure that preflight responses contain a `Vary` header. [#224]

[#224]: https://github.com/actix/actix-extras/pull/224

## 0.6.0-beta.9

- Relax body type bounds on middleware impl. [#223]
- Update `actix-web` dependency to `4.0.0-rc.1`.

[#223]: https://github.com/actix/actix-extras/pull/223

## 0.6.0-beta.8

- Minimum supported Rust version (MSRV) is now 1.54.

## 0.6.0-beta.7

- Update `actix-web` dependency to `4.0.0-beta.15`. [#216]

[#216]: https://github.com/actix/actix-extras/pull/216

## 0.6.0-beta.6

- Fix panic when wrapping routes with dynamic segments in their paths. [#213]

[#213]: https://github.com/actix/actix-extras/pull/213

## 0.6.0-beta.5 _(YANKED)_

- Update `actix-web` dependency to `4.0.0.beta-14`. [#209]

[#209]: https://github.com/actix/actix-extras/pull/209

## 0.6.0-beta.4

- No significant changes since `0.6.0-beta.3`.

## 0.6.0-beta.3

- Make `Cors` middleware generic over body type [#195]
- Fix `expose_any_header` behavior. [#204]
- Update `actix-web` dependency to v4.0.0-beta.10. [#203]
- Minimum supported Rust version (MSRV) is now 1.52.

[#195]: https://github.com/actix/actix-extras/pull/195
[#203]: https://github.com/actix/actix-extras/pull/203
[#204]: https://github.com/actix/actix-extras/pull/204

## 0.6.0-beta.2

- No notable changes.

## 0.6.0-beta.1

- Update `actix-web` dependency to 4.0.0 beta.
- Minimum supported Rust version (MSRV) is now 1.46.0.

</details>

## 0.5.4

- Fix `expose_any_header` method, now set the correct field. [#143]

[#143]: https://github.com/actix/actix-extras/pull/143

## 0.5.3

- Fix version spec for `derive_more` dependency.

## 0.5.2

- Ensure `tinyvec` is using the correct features.
- Bump `futures-util` minimum version to `0.3.7` to avoid `RUSTSEC-2020-0059`.

## 0.5.1

- Fix `allow_any_header` method, now set the correct field. [#121]

[#121]: https://github.com/actix/actix-extras/pull/121

## 0.5.0

- Disallow `*` in `Cors::allowed_origin`. [#114].
- Hide `CorsMiddleware` from docs. [#118].
- `CorsFactory` is removed. [#119]
- The `impl Default` constructor is now overly-restrictive. [#119]
- Added `Cors::permissive()` constructor that allows anything. [#119]
- Adds methods for each property to reset to a permissive state. (`allow_any_origin`, `expose_any_header`, etc.) [#119]
- Errors are now propagated with `Transform::InitError` instead of panicking. [#119]
- Fixes bug where allowed origin functions are not called if `allowed_origins` is All. [#119]
- `AllOrSome` is no longer public. [#119]
- Functions used for `allowed_origin_fn` now receive the Origin HeaderValue as the first parameter. [#120]

[#114]: https://github.com/actix/actix-extras/pull/114
[#118]: https://github.com/actix/actix-extras/pull/118
[#119]: https://github.com/actix/actix-extras/pull/119
[#120]: https://github.com/actix/actix-extras/pull/120

## 0.4.1

- Allow closures to be used with `allowed_origin_fn`. [#110]

[#110]: https://github.com/actix/actix-extras/pull/110

## 0.4.0

- Implement `allowed_origin_fn` builder method. [#93]
- Use `TryInto` instead of `TryFrom` where applicable. [#106]

[#93]: https://github.com/actix/actix-extras/pull/93
[#106]: https://github.com/actix/actix-extras/pull/106

## 0.3.0

- Update `actix-web` dependency to 3.0.0.
- Minimum supported Rust version (MSRV) is now 1.42.0.
- Implement the Debug trait on all public types.

## 0.3.0-alpha.1

- Minimize `futures-*` dependencies
- Update `actix-web` dependency to 3.0.0-alpha.1

## 0.2.0 - 2019-12-20

- Release

## 0.2.0-alpha.3 - 2019-12-07

- Migrate to actix-web 2.0.0
- Bump `derive_more` crate version to 0.99.0

## 0.1.0 - 2019-06-15

- Move cors middleware to separate crate
