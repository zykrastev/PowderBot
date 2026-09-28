# PowderBot core — scale parser

This package has no ESP32 dependencies. The first module parses one scale reply
into an `f32` weight in grains, or returns a `ParseError`.

The initial format comes from the C++ firmware's `-    0.10 GN` example.
It still needs comparison with real scale output. Only uppercase `GN` is
accepted; grams and other units are rejected rather than converted.

## Run on your PC

```sh
cd /data/Projects/PowderBot/core
cargo +stable test
cargo +stable fmt --check
cargo +stable clippy --all-targets -- -D warnings
```

Tests cover positive/negative weights, zero, padding, line endings, missing
fields, incorrect units, malformed numbers, and numeric overflow.

## Rust concepts in this step

- `pub mod scale_protocol` exposes a module from the library.
- `&str` borrows text without copying or allocating it.
- `Result<f32, ParseError>` means success with a number or failure with a reason.
- `enum` defines the possible error variants.
- `?` returns early if an operation fails.
- `match` handles alternatives explicitly.
- `#[test]` marks a function for Cargo's test runner.

The parser doesn't open UART, send scale commands, or change motor state.
Firmware integration comes after the parser's tests have been run.
