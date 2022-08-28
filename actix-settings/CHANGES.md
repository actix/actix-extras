# Changes

## Unreleased - 2022-xx-xx

- Rename `AtError => Error`.
- Remove `AtResult` type alias.
- Minimum supported Rust version (MSRV) is now 1.59 due to transitive `time` dependency.

## 0.6.0 - 2022-07-31

- Update Actix Web dependencies to v4 ecosystem.
- Rename `actix.ssl` settings object to `actix.tls`.
- `NoSettings` is now marked `#[non_exhaustive]`.

## 0.5.2 - 2022-07-31

- Adopted into @actix org from <https://github.com/jjpe/actix-settings>.
