# Borrowed inputs

`EventRef` borrows field text and `Encoder::encode_borrowed` writes it into the caller's buffer. The owned encoder keeps its current implementation. The borrowed encoder uses the existing line and retry helpers, with byte searches to validate its new borrowed fields. Both paths preserve field rules, line endings, and the reservation policy on main.

The benchmarks compare complete owned construction with borrowed construction. `encode_codec_constructed_message` includes three shared `ByteString` clones and drops. Both new-buffer cases include output buffer allocation. `encode_codec_borrowed_message_reused_buffer` measures borrowed encoding with an existing output buffer. Run `just bench-test` to validate benchmark output.

The earlier combined experiments measured the following best times for new output buffers:

| Payload | Owned    | Borrowed | Speedup        |
| ------- | -------- | -------- | -------------- |
| 32 B    | 48 ns    | 41 ns    | 1.17×          |
| 1 KiB   | 73 ns    | 64 ns    | 1.14×          |
| 64 KiB  | 1.826 µs | 1.828 µs | No improvement |

The experiment included other optimizations and preceded [actix-web-lab #532](https://github.com/actix/actix-web-lab/pull/532). The 32 B and 1 KiB median gains were about 2% and 6%. [Exploratory CodSpeed run in actix-web-lab](https://app.codspeed.io/robjtede/actix-web-lab/runs/6accb79e051eb5e27d77602c).

An earlier stacked version routed owned encoding through `EventRef` and regressed small owned events. Its [CI comparison](https://app.codspeed.io/robjtede/actix-web-lab/runs/compare/6accced8095126830ad2bef5..6acccf74095126830ad2bf0f) identified the extra dispatcher and drop work. The independent version keeps the owned implementation and shares the line and retry helpers.
