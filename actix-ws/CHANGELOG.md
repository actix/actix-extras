# Changelog

## Unreleased

- Take the encoded buffer when yielding bytes in the response stream rather than splitting the buffer, reducing memory use
- Remove type parameters from `Session::{text, binary}()` methods, replacing with equivalent `impl Trait` parameters.
- `Session::text()` now receives an `impl Into<ByteString>`, making broadcasting text messages more efficient.
- Allow sending continuations via `Session::continuation()`
- Enable customizing `max_size` for received frames
- Add new ability to aggregate received continuations

## 0.2.5

- Adopted into @actix org from <https://git.asonix.dog/asonix/actix-actorless-websockets>.
