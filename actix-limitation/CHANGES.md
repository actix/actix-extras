# Changes

## Unreleased - 2022-xx-xx
- Updated `session-session` dependency to `0.7`.


## 0.2.0 - 2022-03-22
- Update Actix Web dependency to v4 ecosystem. [#229]
- Update Tokio dependencies to v1 ecosystem. [#229]
- Rename `Limiter::{build => builder}()`. [#232]
- Rename `Builder::{finish => build}()`. [#232]
- Exceeding the rate limit now returns a 429 Too Many Requests response. [#232]

[#229]: https://github.com/actix/actix-extras/pull/229
[#232]: https://github.com/actix/actix-extras/pull/232


## 0.1.4 - 2022-03-18
- Adopted into @actix org from <https://github.com/0xmad/actix-limitation>.
