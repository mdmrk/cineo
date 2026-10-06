# Contributing

Thanks for helping! Start with [AGENTS.md](AGENTS.md). It is the engineering
contract (workflow, invariants, doc sync, commits) for humans and AI agents
alike. This page covers setup, commands and the conventions AGENTS.md links to.

## Setup

1. Install Rust with [rustup](https://rustup.rs). The pinned toolchain in
   `rust-toolchain.toml` (1.99.0, with rustfmt and clippy) installs itself on
   first use. If your rustup does not auto-install it, run
   `rustup toolchain install`.
2. Optional tools, which CI always runs:
   ```sh
   cargo install --locked cargo-nextest cargo-deny
   ```
3. `mpv` on `PATH` to play anything.

If you build with a distro-packaged Rust instead of rustup, make sure the
version matches the pin. Some distro builds expect a cross linker name; if
linking fails with `linker 'x86_64-linux-gnu-gcc' not found`, set
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc` in your shell. Do not commit
that setting.

## Everyday commands

| Task | Command |
|------|---------|
| Everything CI checks | `scripts/check.sh` (`--quick` skips docs and deny) |
| Run the desktop app | `cargo run -p cineo-desktop -- -v` (`--data-dir`, `--cache-dir`, `--allow-private-network`, `--mpv`) |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Test | `cargo nextest run --workspace` or `cargo test --workspace` |
| Doctests (nextest skips them) | `cargo test --workspace --doc` |
| One test | `cargo test -p cineo-core some_name` |
| Docs | `cargo doc --workspace --no-deps --open` |
| Dependency policy | `cargo deny check` |

`--locked` is used everywhere in CI. If you change dependencies, commit the
updated `Cargo.lock`.

## Conventions

- **Commits:** [Conventional Commits](https://www.conventionalcommits.org).
  The scope is the crate name without `cineo-`, or an area (`ci`, `docs`,
  `agents`).
- **Branches:** `feat/<topic>`, `fix/<topic>`, `docs/<topic>`. One topic
  per branch or PR; rebase on `main`. Check for overlapping PRs first.
- **Code style:** rustfmt (edition 2024), the workspace lint set in
  `Cargo.toml`, and the Rust conventions below.
- **Fail loudly.** No silent stubs. Unsupported input produces an error or a
  logged warning.
- Investigation notes and logs belong in the PR discussion, not the
  repository.

## Rust conventions

- Errors: libraries use a `thiserror` enum per failure domain, marked
  `#[non_exhaustive]`, with messages that say what failed. Binaries use
  `anyhow` + `.context()` at layer boundaries. No `unwrap`/`expect`/`panic!`
  on input-dependent paths outside tests.
- Logging: `tracing` only. Libraries never print. Spans carry ids, not
  secrets.
- Ownership: borrow by default; clone only when ownership is needed, never
  just to silence the borrow checker.
- Public API: keep items `pub(crate)` unless another crate needs them. Each
  public type gets a doc comment stating its invariants.
- Domain types enforce invariants in constructors (`new`/`parse` returning
  `Option`/`Result`). Do not pass raw `String`s for ids, types or URLs
  across modules.
- Async only in IO crates and shells (tokio). `cineo-core` stays sync and
  pure.
- Tests: behavior-named, fixtures from `tests/fixtures/`, no network, no
  sleeps ([TESTING.md](docs/TESTING.md)).

## Documentation conventions

- One home per fact. Link instead of duplicating; `AGENTS.md` links, it does
  not explain. Keep `AGENTS.md` under about 100 lines.
- Date research snapshots (versions, external behavior): "as of YYYY-MM-DD".
- Never rewrite an accepted ADR's decision. Supersede it with a new ADR and
  update the index in [DECISIONS.md](docs/DECISIONS.md).
- Do not add documents without a clear reader and purpose.

## Lint policy

- Default clippy groups, plus a small curated set in `[workspace.lints]`:
  - no `unwrap`/`expect` outside tests
  - no `dbg!`/`todo!`
  - no printing from libraries
  - a few clarity lints
- We do **not** enable `pedantic` or `nursery` wholesale. Adding a lint needs
  a reason: it must catch real bugs or enforce a project rule. Remove lints
  that only cause churn.
- `#[allow(...)]` needs a comment explaining why. `#[expect(...)]` is
  preferred when the lint should fire.
- `unsafe_code = "forbid"` (see [SECURITY.md](docs/SECURITY.md) for the
  exception process).

## Toolchain bumps (ADR-0009)

1. Update `channel` in `rust-toolchain.toml` and `rust-version` in
   `Cargo.toml`.
2. Fix new lints.
3. Add a `CHANGELOG.md` entry under *Changed*.

## Adding a dependency

1. Check that it is really needed. Prefer std or an existing dependency.
2. Research it: current version, license, maintenance, MSRV, alternatives.
3. Add it to `[workspace.dependencies]` with minimal features. Crates use
   `dep.workspace = true`.
4. Add a row to [docs/dev/DEPENDENCIES.md](docs/dev/DEPENDENCIES.md).
5. `cargo deny check` must pass. A new license needs a
   [LEGAL.md](docs/LEGAL.md) note.

## Pull requests

- Run `scripts/check.sh` before opening one.
- **AI assistance** must be disclosed in the PR description. You are
  responsible for every line you submit.

By contributing, you agree that your contributions are licensed under the
project's MIT license.
