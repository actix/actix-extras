# Changelog

## Unreleased

- Ensure TCP connection is properly shut down when session is dropped.

## 0.3.0

- Add `AggregatedMessage[Stream]` types.
- Add `MessageStream::max_frame_size()` setter method.
- Add `Session::continuation()` method.
- The `Session::text()` method now receives an `impl Into<ByteString>`, making broadcasting text messages more efficient.
- Remove type parameters from `Session::{text, binary}()` methods, replacing with equivalent `impl Trait` parameters.
- Reduce memory usage by `take`-ing (rather than `split`-ing) the encoded buffer when yielding bytes in the response stream.

## 0.2.5

- Adopted into @actix org from <https://git.asonix.dog/asonix/actix-actorless-websockets>.
