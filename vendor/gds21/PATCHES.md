# Local patches

This is `gds21` 0.2.0 from crates.io.

- Chrono's clock support is disabled so the crate can target Typst's
  freestanding WebAssembly environment.
- Default GDS timestamps are deterministic instead of reading the system clock.
