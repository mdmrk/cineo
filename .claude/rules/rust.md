---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---

# Rust conventions

- Edition 2024, pinned toolchain. Workspace lints live in the root
  `Cargo.toml`. Do not weaken them per crate; a local `#[allow]` or
  `#[expect]` needs a comment with the reason.
- Errors:
  - Libraries use a `thiserror` enum per failure domain, marked
    `#[non_exhaustive]`, with messages that say what failed.
  - Binaries use `anyhow` + `.context()` at layer boundaries.
  - No `unwrap`/`expect`/`panic!` on input-dependent paths outside tests.
- No `unsafe` (`forbid`). The exception process is in `docs/SECURITY.md`.
- Logging: `tracing` only. Libraries never print. Spans carry ids, not
  secrets.
- Ownership: borrow by default. Clone only when ownership is needed. Do not
  `.clone()` to silence the borrow checker without understanding why.
- Public API: keep items `pub(crate)` unless another crate needs them. Each
  public type gets a doc comment stating its invariants.
- Domain types enforce invariants in constructors (`new`/`parse` returning
  `Option`/`Result`). Do not pass raw `String`s for ids, types or URLs
  across modules.
- Async: only in IO crates and shells (tokio). `cineo-core` stays sync and
  pure.
- Dependencies: add only via `[workspace.dependencies]`, with minimal
  features, after the research steps in `docs/DEVELOPMENT.md`.
- Tests: behavior-named; fixtures from `tests/fixtures/`; no network, no
  sleeps. Test crates may `#![allow(clippy::unwrap_used, clippy::expect_used)]`.
