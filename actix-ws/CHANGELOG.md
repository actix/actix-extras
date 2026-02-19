# Changelog

## Unreleased

- feat: Add `handle_with_protocols()` for `Sec-WebSocket-Protocol` negotiation [#479]
- feat: Add optional typed message codecs with serde_json support.
- feat: Implement `Sink<Message>` for `Session`
- fix: Ignore empty continuation chunks [#660]
- fix: Truncate oversized control-frame payloads to avoid emitting invalid frames [#508]
- fix: Fix continuation overflow handling

[#479]: https://github.com/actix/actix-extras/issues/479
[#660]: https://github.com/actix/actix-extras/pull/660
[#508]: https://github.com/actix/actix-extras/issues/508

## 0.3.1

- enable actix-web's `ws` feature explicitly.
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
