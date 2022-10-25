# Changes

## Unreleased - 2022-xx-xx

- Minimum supported Rust version (MSRV) is now 1.59 due to transitive `time` dependency.
- Replace use of anyhow::Error with `IdentityError`

## 0.5.2 - 2022-07-19

- Fix visit deadline. [#263]

[#263]: https://github.com/actix/actix-extras/pull/263

## 0.5.1 - 2022-07-11

- Remove unnecessary dependencies. [#259]

[#259]: https://github.com/actix/actix-extras/pull/259

## 0.5.0 - 2022-07-11

`actix-identity` v0.5 is a complete rewrite. The goal is to streamline user experience and reduce maintenance overhead.

`actix-identity` is now designed as an additional layer on top of `actix-session` v0.7, focused on identity management. The identity information is stored in the session state, which is managed by `actix-session` and can be stored using any of the supported `SessionStore` implementations. This reduces the surface area in `actix-identity` (e.g., it is no longer concerned with cookies!) and provides a smooth upgrade path for users: if you need to work with sessions, you no longer need to choose between `actix-session` and `actix-identity`; they work together now!

`actix-identity` v0.5 has feature-parity with `actix-identity` v0.4; if you bump into any blocker when upgrading, please open an issue.

Changes:

- Minimum supported Rust version (MSRV) is now 1.57 due to transitive `time` dependency.
- `IdentityService`, `IdentityPolicy` and `CookieIdentityPolicy` have been replaced by `IdentityMiddleware`. [#246]
- Rename `RequestIdentity` trait to `IdentityExt`. [#246]
- Trying to extract an `Identity` for an unauthenticated user will return a `401 Unauthorized` response to the client. Extract an `Option<Identity>` or a `Result<Identity, actix_web::Error>` if you need to handle cases where requests may or may not be authenticated. [#246]

  Example:

  ```rust
  use actix_web::{http::header::LOCATION, get, HttpResponse, Responder};
  use actix_identity::Identity;

  #[get("/")]
  async fn index(user: Option<Identity>) -> impl Responder {
      if let Some(user) = user {
          HttpResponse::Ok().finish()
      } else {
          // Redirect to login page if unauthenticated
          HttpResponse::TemporaryRedirect()
              .insert_header((LOCATION, "/login"))
              .finish()
      }
  }
  ```

[#246]: https://github.com/actix/actix-extras/pull/246

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
