# AGENTS.md — engineering contract

Cineo: an independent Rust desktop media center for Stremio-protocol addons.
Goals and scope: `docs/GOALS.md`. Current state: `docs/ROADMAP.md` (the
milestone table) and `docs/COMPATIBILITY.md`.

## Architecture invariants (docs/ARCHITECTURE.md, docs/ADR/)
- Dependency direction: shells → IO crates → `cineo-core`. **`cineo-core`
  does no IO**: no network, filesystem, clock or async runtime. Time and IO
  results are inputs; effects are returned as data (ADR-0001).
- New crate = new IO/dependency boundary only (ADR-0002). A new concept is a
  module.
- Addon data is untrusted. Lenient field parsing with recorded warnings →
  strict domain types (ADR-0003). Follow the reference-client semantics in
  `docs/ADDON_PROTOCOL.md`.
- All HTTP goes through `cineo-net` and its `NetPolicy` (ADR-0006). The
  player only receives typed commands (ADR-0004).
- Changing any of these requires a new or updated ADR **before** the code.

## Workflow
1. Understand → read the relevant docs and code. Do not guess current
   behavior; check it.
2. Plan. For multi-file or behavior changes, write a short plan (files,
   tests, docs, ADR impact) and get it approved.
3. Implement the smallest vertical slice. No unrelated refactors, renames
   or reformatting.
4. Test. Every behavior change gets a test; every bug fix gets a regression
   test written first.
5. Verify with `scripts/check.sh` (or `--quick` while iterating). It mirrors
   CI.
6. Review your own diff: scope, error handling, security, tests, docs.
7. Update the docs in the same change (below), then summarize what you
   changed and **how you verified it**.

## Commands
- `scripts/check.sh` — all gates: fmt, clippy `-D warnings`, tests, docs,
  cargo-deny.
- `cargo test -p <crate> <name>` — a single test.
- `cargo fmt --all` — formatting (rustfmt, edition 2024).

## Documentation sync (mandatory, same change)
- User-visible or protocol behavior → `docs/COMPATIBILITY.md` (status only
  goes up with a named test) and `docs/ADDON_PROTOCOL.md`.
- Architecture → ADR + `docs/ARCHITECTURE.md` + this file if an invariant
  changed.
- Security-relevant defaults → `docs/SECURITY.md`.
- New dependency → `docs/dev/DEPENDENCIES.md` (+ `docs/LEGAL.md` for a new
  license).
- Known limitation left in place → `docs/dev/TECHNICAL_DEBT.md`.
- User-visible change → `CHANGELOG.md` under *Unreleased*.

## Honesty rules
- Label claims **VERIFIED** (tested or observed), **INFERRED** (reasoned) or
  **UNKNOWN**. Never present inferred behavior as fact.
- Docs describe what exists and is tested, not intentions.
- Research current versions and docs (crates.io, official docs) before
  choosing dependencies or relying on tool behavior. Do not answer from
  memory.
- Do not copy code from stremio-web (GPL-2.0) or stremio-shell-ng (no
  license). Record any adapted code in `docs/LEGAL.md`.

## Commits
Conventional Commits (`feat(core): …`, `fix(net): …`, `docs: …`, `ci: …`).
One topic per commit or PR.
