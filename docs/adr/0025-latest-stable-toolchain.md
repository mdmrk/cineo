# 0025. Latest stable toolchain; no pin and no MSRV

- Status: Accepted
- Date: 2026-10-27
- Supersedes: 0009

## Context
ADR-0009 pinned Rust 1.99.0 in `rust-toolchain.toml` and set
`rust-version = "1.99"` in the workspace. The owner asked to remove the
toolchain version requirement (owner decision 2026-10-27). Cineo is an
application, not a published library, so no downstream crate depends on a
minimum Rust version.

## Options considered
1. Keep the pin — reproducible lints, but every contributor and CI must
   install that exact toolchain, and each bump is a manual change.
2. Build with the latest stable Rust — no pin to maintain; new stable lints
   or formatting can show up without a code change.

## Decision
- No `rust-toolchain.toml` and no `rust-version` in any `Cargo.toml`.
- CI and release builds install the latest `stable` with rustfmt and clippy.
- Contributors use a current stable Rust.

## Consequences
No toolchain bumps to maintain. A new stable release can fail CI with new
clippy lints; fix them when they appear. An old toolchain fails with a
compiler or dependency error instead of Cargo's `rust-version` message
(INFERRED). Edition 2024 needs Rust 1.85 or newer, and some dependencies
need newer still (see DEVELOPMENT.md#dependencies).

## Rejected alternatives
Keeping the pin: the owner asked to remove it.
