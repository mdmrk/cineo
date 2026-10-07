# 0009. Pinned stable toolchain; MSRV equals the pin

- Status: Accepted
- Date: 2026-10-06

## Context
Current stable Rust is 1.99.0 (2026-09-28; VERIFIED from static.rust-lang.org channel manifest). Edition 2024 needs ≥ 1.85. stremio-core keeps a low MSRV (1.77) and a separate MSRV CI job because it is consumed as a library (VERIFIED-REF). Cineo is an application; candidate dependencies already require 1.85–1.95.

## Decision
`rust-toolchain.toml` pins `1.99.0` (rustfmt, clippy). `rust-version = "1.99"` in the workspace. No separate MSRV job. Bump deliberately (both files) roughly each 1–2 stable releases or when a dependency needs it.

## Consequences
Reproducible local/CI builds; one toolchain to support. Contributors with older toolchains get a clear error. Revisit if a crate is ever published for external use.

## Rejected alternatives
Floating `stable` (non-reproducible lint/format changes); a lower MSRV (cost without users).
