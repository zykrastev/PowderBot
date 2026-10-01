# PowderBot core

Hardware-independent scale parsing, dispensing control, profiles, and storage.

```sh
cargo +stable test
cargo +stable fmt --check
cargo +stable clippy --all-targets -- -D warnings
```

Run from this directory. Keep the scale set to grains; unitless readings are
interpreted as GN.
