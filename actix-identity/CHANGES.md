# Changes

## Unreleased - 2022-xx-xx
- Minimum supported Rust version (MSRV) is now 1.57 due to transitive `time` dependency.


## 0.4.0 - 2022-03-01
- Update `actix-web` dependency to `4`.


## 0.4.0-beta.9 - 2022-02-07
- Relax body type bounds on middleware impl. [#223]
- Update `actix-web` dependency to `4.0.0-rc.1`.

[#223]: https://github.com/actix/actix-extras/pull/223


## 0.4.0-beta.8 - 2022-01-21
- No significant changes since `0.4.0-beta.7`.


## 0.4.0-beta.7 - 2021-12-29
- Update `actix-web` dependency to `4.0.0.beta-18`. [#218]
- Minimum supported Rust version (MSRV) is now 1.54.

[#218]: https://github.com/actix/actix-extras/pull/218


## 0.4.0-beta.6 - 2021-12-18
- Update `actix-web` dependency to `4.0.0.beta-15`. [#216]

[#216]: https://github.com/actix/actix-extras/pull/216


## 0.4.0-beta.5 - 2021-12-12
- Update `actix-web` dependency to `4.0.0.beta-14`. [#209]

[#209]: https://github.com/actix/actix-extras/pull/209


## 0.4.0-beta.4 - 2021-11-22
- No significant changes since `0.4.0-beta.3`.


## 0.4.0-beta.3 - 2021-10-21
- Update `actix-web` dependency to v4.0.0-beta.10. [#203]
- Minimum supported Rust version (MSRV) is now 1.52.

[#203]: https://github.com/actix/actix-extras/pull/203


## 0.4.0-beta.2 - 2021-06-27
- No notable changes.


## 0.4.0-beta.1 - 2021-04-02
- Rename `CookieIdentityPolicy::{max_age => max_age_secs}`. [#168]
- Rename `CookieIdentityPolicy::{max_age_time => max_age}`. [#168]
- Update `actix-web` dependency to 4.0.0 beta.
- Minimum supported Rust version (MSRV) is now 1.46.0.

[#168]: https://github.com/actix/actix-extras/pull/168


## 0.3.1 - 2020-09-20
- Add method to set `HttpOnly` flag on cookie identity. [#102]

[#102]: https://github.com/actix/actix-extras/pull/102


## 0.3.0 - 2020-09-11
- Update `actix-web` dependency to 3.0.0.
- Minimum supported Rust version (MSRV) is now 1.42.0.


## 0.3.0-alpha.1 - 2020-03-14
- Update the `time` dependency to 0.2.7
- Update the `actix-web` dependency to 3.0.0-alpha.1
- Minimize `futures` dependency


## 0.2.1 - 2020-01-10
- Fix panic with already borrowed: BorrowMutError #1263


## 0.2.0 - 2019-12-20
- Use actix-web 2.0


## 0.1.0 - 2019-06-xx
- Move identity middleware to separate crate
