# 0038. The UniFFI binding generator in its own crate

- Status: Accepted
- Date: 2026-11-17

## Context
Gradle's `uniffiBindgen` task writes the Kotlin bindings from the Android
library's metadata. It ran `cargo run -p cineo-android --features bindgen
--bin uniffi-bindgen`, and a binary needs its package's library, so every
Android build also compiled all of `cineo-android` for the host (tokio,
librqbit, the player and the store) only to get UniFFI's command-line
tool (INFERRED from how Cargo builds a package's binaries; the build-audit
finding of 2026-11-17).

Measured on 2026-11-17 (12 CPUs, clean target directory, Rust 1.100.0):
- `cargo build -p cineo-android --features bindgen --bin uniffi-bindgen`:
  301 packages, 115 s (VERIFIED).
- The same binary in a crate that depends only on `uniffi` with `cli` and
  `cargo-metadata`: 74 packages, 53 s (VERIFIED).

The generator still reads `crates/cineo-android/uniffi.toml` through
`cargo metadata`, because it runs from the workspace (VERIFIED: the
generated Kotlin has `package io.github.mdmrk.cineo.rust`).

## Options considered
1. **Keep the binary in `cineo-android`.** No new crate; every Android
   build compiles the app's Rust twice, once for the host.
2. **A tool crate, `cineo-uniffi-bindgen`, with only the binary.** Half
   the host build; one more crate, which holds no app code.
3. **`cargo install uniffi-bindgen`** in CI and the F-Droid recipe. No
   crate, but the tool's version would no longer come from `Cargo.lock`.

## Decision
Option 2. `crates/cineo-uniffi-bindgen` holds the `uniffi-bindgen` binary
and depends only on `uniffi` with `cli` and `cargo-metadata`.
`cineo-android` keeps `uniffi` without default features and has no
`bindgen` feature or binary. Gradle runs
`cargo run -p cineo-uniffi-bindgen --bin uniffi-bindgen`.

This is a dependency boundary in the sense of ADR-0002: the generator's
dependencies stay out of the app's crates.

## Consequences
- Android builds compile about 230 fewer host packages.
- No app code may go into `cineo-uniffi-bindgen`; it stays one `main`.
- `docs/ARCHITECTURE.md` lists the crate; the tool's version still comes
  from `uniffi` in `Cargo.lock`.

## Rejected alternatives
- Option 1: it keeps the slow build for nothing.
- Option 3: a second source for the UniFFI version, which must match the
  `uniffi` the library was built with.
