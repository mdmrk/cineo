# 0002. Minimal workspace; crates by IO boundary

- Status: Accepted
- Date: 2026-10-06

## Context
Micro-crates (`catalog`, `metadata`, `library`, …) add build and navigation overhead and invite premature abstraction. stremio-core is effectively one crate plus helpers (VERIFIED-REF). We still need the core to be provably free of IO and UI dependencies.

## Options considered
1. One crate for everything.
2. Crate per domain concept.
3. Crate per **dependency/IO boundary**: pure core; one crate per IO technology; one per shell.

## Decision
Option 3, created lazily: `cineo-core` now (placeholder); `cineo-net`, `cineo-cli` in M1; `cineo-player-mpv` in M3; `cineo-store` in M4; `cineo-desktop` in M5. Dependency direction: shells → IO crates → core. IO crates do not depend on each other. Concepts are modules, not crates. Workspace dependencies and lints are centralized in the root `Cargo.toml`.

## Consequences
- The compiler enforces "core has no IO": it simply has no IO dependencies. Adding one to `cineo-core` is an architecture change requiring an ADR.
- Few crates to navigate; agents can find code by boundary.

## Rejected alternatives
- Option 1 loses the compile-time guarantee. Option 2 multiplies crates without separating dependencies.
