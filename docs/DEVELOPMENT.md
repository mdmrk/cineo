# Development

How to build, test, debug and release Cineo. Start with
[AGENTS.md](../AGENTS.md): it is the engineering contract (workflow,
invariants, doc sync, commits) for humans and AI agents alike. This page
holds the details it links to.

- [Setup](#setup) · [Everyday commands](#everyday-commands) ·
  [Conventions](#conventions) · [Testing](#testing) ·
  [Debugging](#debugging) · [Manual GUI test](#manual-gui-test) ·
  [Dependencies](#dependencies) · [Releasing](#releasing) ·
  [Pull requests](#pull-requests)

## Setup

1. Install Rust with [rustup](https://rustup.rs). The pinned toolchain in
   `rust-toolchain.toml` (1.99.0, with rustfmt and clippy) installs itself on
   first use. If your rustup does not auto-install it, run
   `rustup toolchain install`.
2. Optional tools, which CI always runs:
   ```sh
   cargo install --locked cargo-nextest cargo-deny
   ```
3. libmpv (mpv 0.35 or newer) to play anything; the app loads it at
   runtime (ADR-0014).

If you build with a distro-packaged Rust instead of rustup, make sure the
version matches the pin. Some distro builds expect a cross linker name; if
linking fails with `linker 'x86_64-linux-gnu-gcc' not found`, set
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc` in your shell. Do not commit
that setting.

## Everyday commands

| Task | Command |
|------|---------|
| Everything CI checks | `scripts/check.sh` (`--quick` skips docs and deny) |
| Run the desktop app | `cargo run -p cineo-desktop -- -v` (`--data-dir`, `--cache-dir`, `--allow-private-network`) |
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Test | `cargo nextest run --workspace` or `cargo test --workspace` |
| Doctests (nextest skips them) | `cargo test --workspace --doc` |
| One test | `cargo test -p cineo-core some_name` |
| Docs | `cargo doc --workspace --no-deps --open` |
| Dependency policy | `cargo deny --config .config/deny.toml check` |

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

### Rust

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
- Comments: few. A short line only where the code cannot say it: `SAFETY`,
  security, a non-obvious external behavior, or an `#[allow]` reason.
  Doc comments on public items stay one short paragraph.
- Domain types enforce invariants in constructors (`new`/`parse` returning
  `Option`/`Result`). Do not pass raw `String`s for ids, types or URLs
  across modules.
- Async only in IO crates and shells (tokio). `cineo-core` stays sync and
  pure.
- Tests: behavior-named, fixtures from `tests/fixtures/`, no network, no
  sleeps ([Testing](#testing)).

### Documentation

- One home per fact. Link instead of duplicating; `AGENTS.md` links, it does
  not explain. Keep `AGENTS.md` under about 100 lines.
- Date research snapshots (versions, external behavior): "as of YYYY-MM-DD".
- Never rewrite an accepted ADR's decision. Supersede it with a new ADR and
  update the index in [adr/README.md](adr/README.md).
- Do not add documents without a clear reader and purpose.

### Lint policy

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
- `unsafe_code = "forbid"`, except in two modules of `cineo-player` (see [SECURITY.md](SECURITY.md) for the
  exception process).

### Toolchain bumps (ADR-0009)

1. Update `channel` in `rust-toolchain.toml` and `rust-version` in
   `Cargo.toml`.
2. Fix new lints.
3. Add a `CHANGELOG.md` entry under *Changed*.

## Testing

**Principle:** a behavior is not supported until a test proves it. Bug fixes
start with a failing regression test. Add test infrastructure only when the
first test needs it.

### Pyramid

```
            ┌───────────────┐  few: E2E (CLI binary vs mock addon; later GUI smoke)
          ┌─┴───────────────┴─┐  some: integration (IO crate vs local mock / temp DB / real libmpv, opt-in)
      ┌───┴───────────────────┴───┐  many: unit + protocol fixtures (pure core, fast)
```

| Layer | Where | Tools | Runs in CI |
|-------|-------|-------|-----------|
| Unit | `#[cfg(test)] mod tests` next to the code | std test harness | always |
| Protocol fixtures | `crates/cineo-core/tests/` + `tests/fixtures/addons/` | JSON fixtures | always |
| Network integration | `crates/cineo-net/tests/` | local mock HTTP server (`wiremock`, chosen in M1) | always |
| Persistence | `crates/cineo-store/tests/` | temp-dir SQLite, fixture DBs per schema version | always (M4+) |
| Player | `crates/cineo-player/src/` (unit tests) | pure command, track-list and event-mapping tests; real libmpv tests marked `#[ignore]` (`cargo test -p cineo-player -- --ignored`) | pure: always; real: nightly/manual |
| E2E | `crates/cineo-cli/tests/` | built binary (`CARGO_BIN_EXE_cineo`) + mock addon | always |
| Live addons | `scripts/` (manual) | real public addons | never in CI (flaky, external) |

### Rules

- **Name tests after the behavior**:
  `full_resource_without_types_matches_nothing`, not `test_parse_3`. For
  protocol tests, link the matching ADDON_PROTOCOL.md section in a comment.
- **Fixtures, not inline JSON**, for anything representing a real protocol
  document. Inline JSON is fine for one-field edge cases.
- **No network in automated tests.** Mock servers bind loopback, and tests
  opt into `allow_private_networks` explicitly. One test must keep proving
  that the default policy blocks loopback.
- **No sleeps for synchronization.** Use timeouts on futures. A test that
  checks a timeout uses a short configured timeout (milliseconds).
- **Deterministic.** No wall-clock time in the core. Time is passed in as
  data. Randomness is seeded.
- Panicking helpers (`unwrap`, `expect`) are fine in tests. Test crates may
  allow those lints at the top.
- Assertions on warnings compare **locations**, so that wording changes do
  not break tests.

### Not set up yet

Added only with a concrete first use: property tests (`proptest`, e.g. URL
encoding round-trips), fuzzing (`cargo-fuzz`; targets: the manifest, catalog,
meta and stream parsers and mpv's track-list JSON; nightly, not per PR), snapshot
tests (`insta`, only for large structured output), benchmarks (only for a
measured problem), and a replayed compatibility corpus (v1.0 goal).

Commands are under [Everyday commands](#everyday-commands).

### Per-subsystem strategy

| Subsystem | Strategy |
|-----------|----------|
| Protocol parsing | Fixtures for basic, quirks and invalid cases; every lenient rule has a fixture case; fuzz later |
| URL building | Table tests including the docs' example, reserved chars, unicode, prefix and query transport URLs |
| Filtering/planning | Fixture manifests × request matrix |
| Network policy | Pure IP/URL classification tables + mock-server tests (blocked loopback, redirects, size, gzip bomb, timeout, no URL leak in errors) |
| Aggregation (M2) | Reducer tests: effects emitted, partial failure, stale responses dropped |
| Player (M3, ADR-0014) | Commands built from typed values; event mapping tables; real-libmpv playback `#[ignore]`; controls via kittest |
| Persistence (M4) | Round-trip; migration from each historical schema fixture; corruption handling |
| GUI (M5) | Core-level tests carry the logic; UI smoke test only |

## Debugging

Failures must be observable without a debugger. This page says what each
subsystem logs and how to diagnose it.

### Logging

- `tracing` everywhere. Libraries never print. Only shells write to
  stdout/stderr.
- CLI: `-v` gives debug logs for Cineo crates, `-vv` gives trace for
  everything. `RUST_LOG` overrides both, for example
  `RUST_LOG=cineo_net=trace`. Logs go to **stderr**, so stdout stays
  machine-readable.
- GUI: `cineo-desktop` logs to stderr with the same `-v`/`-vv`/
  `RUST_LOG` rules. libmpv's own messages use the target `libmpv`: its
  errors are always shown, its warnings (about the media, some on every
  frame) only with `-v`. A log file and a diagnostics view are not implemented
  yet ([TECHNICAL_DEBT.md](TECHNICAL_DEBT.md)).

#### Span conventions

| Span | Fields | Emitted by |
|------|--------|-----------|
| `addon_request` | `req` (process-unique id), `addon` (origin only), `resource` (encoded resource path) | `cineo-net` |
| `image_request` | `req`, `origin` | `cineo-net` |

**Never log** full transport URLs, query strings of addon URLs, stream URLs
with tokens, or headers from `proxyHeaders`. Log the origin and path instead.

### Diagnosing

#### Addon failures

1. `cineo -v addon inspect <manifest-url>`. This shows the parsed
   manifest, every **warning** (field location + reason), and the request
   span.
2. Read the error type. It is one of:
   - `blocked by network policy` (private address, redirect downgrade, too
     many redirects)
   - `timed out`
   - `HTTP <status>`
   - `exceeds the N-byte limit`
   - `invalid manifest: …` (the field is named)
3. If another client accepts the addon and Cineo does not, compare against
   [ADDON_PROTOCOL.md](ADDON_PROTOCOL.md). Then file an "Addon
   compatibility" issue with the inspect output.

#### Catalog / metadata failures

- `cineo -v catalog <url> <type> <id> [--extra k=v]`. It refuses
  undeclared catalogs and invalid extras **before** sending a request, and
  the message says which rule failed.
- Skipped items appear as `warn` lines with `metas[i]` locations.
- An empty result with no warnings means the addon returned
  `"metas": []` or `null`.

#### Stream failures

- Streams load per addon. A failing addon shows its own error and does not
  hide the others.
- Torrent streams go through `cineo-stream`; they show a notice instead when
  P2P is turned off in Settings. Other non-`http(s)` sources (archives, NZB,
  …) are listed but refused with a notice naming the source kind.
- Logs: `RUST_LOG=cineo_net=debug` shows each `addon_request` span with its
  resource path; `RUST_LOG=cineo_stream=debug` shows player connections and
  proxy handshakes of the torrent engine.

#### Playback failures

- With debug logging (`RUST_LOG=cineo_player=debug`), every command sent
  to mpv is logged, and so is which libmpv was loaded.
- mpv's own warnings and errors are logged under the `libmpv` target. To
  debug mpv further, reproduce by running `mpv <url>` directly.
- Common causes: an unsupported codec (check `hwdec` fallback in the mpv log),
  an HTTP 403 (check `proxyHeaders`), an expired stream URL.

#### Persistence problems

- `cineo doctor` prints the database path, schema version, row counts and an
  integrity-check result.
- Migrations log `from → to` at `info`. All pending migrations run in one
  transaction, so a failed migration leaves the database at its previous
  version.
- A corrupt database or one from a newer Cineo is reported and never
  modified. To start over, move `cineo.db` away (keep it for the report).

#### UI / core synchronization

- Every action and every effect result goes through one dispatch function,
  which logs at `debug` with a sequence number. A UI showing stale data means
  a missing state emission. Check that the reducer test covers the action.

### Reproducible bug reports

A good report contains:
1. The exact command or UI steps.
2. `-v` logs, scrubbed of personal data.
3. The version (`cineo --version`) and OS.
4. For addon issues: a **public** manifest URL only.

The issue templates ask for these.

## Manual GUI test

The UI tests (`crates/cineo-desktop/tests/ui.rs`) check rendering and the
actions clicks produce. This script covers what they cannot: real windows,
real addons, libmpv, and restarts. Run it before marking a GUI milestone done,
on **Linux and Windows**, and record the date, OS and result in
[ROADMAP.md](ROADMAP.md) or the PR.

### Setup

- Build: `cargo build -p cineo-desktop --release`. libmpv (mpv 0.35 or
  newer) must be installed for playback.
- Use a throwaway data directory: `--data-dir <tmp>`.
- A stream addon that returns direct `http(s)` URLs. Without one, use this
  local addon (only for testing; the video must be a file you may use):

  ```sh
  mkdir -p addon/stream/movie && cd addon
  cp /path/to/video.mp4 video.mp4
  cat > manifest.json <<'JSON'
  {"id":"org.cineo.manual","version":"1.0.0","name":"Manual Test Streams",
   "resources":["stream"],"types":["movie"],"idPrefixes":["tt"],"catalogs":[]}
  JSON
  # tt0063350 = Night of the Living Dead (1968) in Cinemeta
  echo '{"streams":[{"name":"Local","url":"http://127.0.0.1:8000/video.mp4"}]}' \
    > stream/movie/tt0063350.json
  python3 -m http.server 8000
  ```

  Then start Cineo with `--allow-private-network` so it may reach
  `127.0.0.1`.

### Steps (success scenario 1 in ROADMAP.md)

| # | Do | Expect |
|---|----|--------|
| 1 | Start `cineo-desktop --data-dir <tmp> -v` | Empty Board with "No addons installed…" |
| 2 | Addons → paste `https://v3-cinemeta.strem.io/manifest.json` → Install | "Installed Cinemeta"; Board shows rows with posters |
| 3 | Install the stream addon (`http://127.0.0.1:8000/manifest.json` for the local one) | It appears second in the list; Move up/down reorders |
| 4 | Search "Night of the Living Dead" → open the 1968 film | Detail page: backdrop, poster, facts, description |
| 5 | Streams list | One group per stream addon; the local stream has an enabled Play |
| 6 | Play | The player opens in the window and plays. Total time from step 2: under a minute |
| 7 | Seek to ~10 minutes, wait 10 s, press Back | Board shows "Continue watching" with a progress bar |
| 8 | Quit Cineo, start it again with the same `--data-dir` | Addons (in order) and Continue watching are back |
| 9 | Open the item from Continue watching → Play | Playback resumes at ~10 minutes |
| 10 | Discover → pick "Popular — Cinemeta", a genre, "Load more" | Items change with the genre; more items append |
| 11 | Open a series → choose a season → an episode | Episode list for that season; streams load for the episode |
| 12 | Torrent streams | See [Torrents (M9)](#torrents-m9) |
| 13 | Library → Remove the item | It disappears, and stays gone after a restart |
| 14 | Install `http://127.0.0.1:9/manifest.json` without `--allow-private-network` | Error mentions the network policy; nothing crashes |

Also watch the `-v` log: no panics, and no full addon or stream URLs (only
origins and resource paths).

### Torrents (M9)

Use a torrent of content you may share (for example a public-domain film
from the Internet Archive; each item has a torrent whose info hash is on
its page). Add a stream to the local addon above, in front of the
`Local` entry:

```json
{"name":"Torrent","infoHash":"<40 hex characters>","fileIdx":0}
```

Use a fresh `--data-dir` and `--cache-dir` so the P2P notice appears.

| # | Do | Expect |
|---|----|--------|
| T1 | Open the film; Play the torrent stream | The "Peer-to-peer streaming" notice; nothing appears in the cache directory yet |
| T2 | Cancel | No playback, no engine log lines (`-v`) |
| T3 | Play again → Accept and play | The player opens at once with Back and the film's logo (or, without one, its title) pulsing in the middle; the video starts once the engine serves the file |
| T4 | Seek forward and back | Playback resumes within a few seconds |
| T5 | Back (or Esc) | The detail page returns; the log shows the torrent stopped. Back while still connecting cancels the torrent too |
| T6 | Play it again | No notice this time. While it plays, the cache directory holds its `<info hash>` folder with only `<file index>` files (no torrent file names) |
| T7 | Settings → untick "Show and play torrent streams"; open the film | The torrent stream is gone; "1 torrent stream hidden…" is shown |
| T8 | Restart with the same directories | The setting is kept |
| T9 | Back, then look in the cache directory | Only `dht.json`: the torrent's data was deleted when it stopped |

## Dependencies

1. Check that it is really needed. Prefer std or an existing dependency.
2. Research it: current version, license, maintenance, MSRV, alternatives.
3. Add it to `[workspace.dependencies]` with minimal features. Crates use
   `dep.workspace = true`.
4. Add a row to the *In use* table below.
5. `cargo deny --config .config/deny.toml check` must pass. A new license needs a
   [LEGAL.md](LEGAL.md) note.

Updates are manual (`cargo update`, then `scripts/check.sh`); there is
no Dependabot, so no bot branches.

Every direct dependency gets a row **when it is added**. Research
snapshot: crates.io, 2026-10-06. Versions are the latest stable at that date.
Re-check them when adding.

### In use

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| `percent-encoding` | 2.3.2 | `encodeURIComponent` path encoding (core) | MIT/Apache |
| `semver` | 1.0.28 | Manifest `version` (core) | MIT/Apache |
| `serde_json` | 1.0.151 | JSON documents (core) | MIT/Apache |
| `thiserror` | 2.0.21 | Library errors | MIT/Apache |
| `url` | 2.5.8 | URLs (core, net) | MIT/Apache |
| `reqwest` | 0.13.5 | Addon HTTP client (net) | MIT/Apache |
| `tokio` | 1.53.2 | Async runtime (net, shells) | MIT |
| `tracing` / `tracing-subscriber` | 0.1.44 / 0.3.23 | Logging | MIT |
| `anyhow` | 1.0.104 | Shell errors (cli) | MIT/Apache |
| `clap` | 4.6.7 | CLI arguments (cli) | MIT/Apache |
| `wiremock` (dev) | 0.6.5 | Mock addon servers in tests | MIT/Apache |
| `rusqlite` (`bundled`, no default features) | 0.40.2 | SQLite database (store) | MIT; bundled SQLite is public domain |
| `etcetera` | 0.11.0 | Platform data directory (store) | MIT/Apache |
| `eframe` (`glow`, `default_fonts`, `x11`, `wayland`, `accesskit`; no default features) | 0.36.2 | Desktop window and egui (desktop, ADR-0011) | MIT/Apache |
| `iconflow` (`pack-tabler` only; no default features) | 2.1.0 | Tabler icon font and codepoints for the desktop UI (desktop) | MIT; Tabler font MIT |
| `image` (`jpeg`, `png`, `webp` only) | 0.25.10 | Image header checks, decoding and downscaling (desktop) | MIT/Apache |
| `egui_kittest` (dev, no default features) | 0.36.2 | Headless UI tests via AccessKit (desktop) | MIT/Apache |
| `librqbit` (`rust-tls`, no default features) | 9.0.1 + upstream `main` and our patch, from the [fork](https://github.com/mdmrk/rqbit/tree/fix/urgent-piece-helpers) at `be52c57a` | BitTorrent session (stream, ADR-0012) | Apache-2.0 |
| `hyper` (`server`, `http1`) / `hyper-util` (`tokio`) / `http-body-util` | 1.11.1 / 0.1.21 / 0.1.5 | Loopback HTTP server for the player (stream) | MIT |
| `bytes` | 1.12.1 | Response bodies (stream) | MIT |
| `futures-util` (no default features) | 0.3.34 | Stream adapters for bodies (stream) | MIT/Apache |
| `tokio-util` (`io`) | 0.7.19 | `ReaderStream` from a torrent file reader (stream) | MIT |
| `getrandom` | 0.4.3 | Per-session path token and proxy password (stream) | MIT/Apache |
| `libloading` | 0.8.9 | Loads libmpv at runtime for embedded playback (player, ADR-0014). 0.9.0 exists (as of 2026-10-07); 0.8.9 is already in the tree via glutin, so we avoid a second copy | ISC |

`tokio` features `io-util` and `sync` were added for the mpv player (M3);
`process` was dropped with the external mpv player (2026-10-07). The player
uses `serde_json` for mpv's track list.

`rusqlite` drops its default features (`cache`, and an FFI backend used only
on wasm). `bundled` compiles SQLite from source, so no system library is
needed on any OS. `etcetera` MSRV: 1.87 (M4).

M5 (2026-10-06): the egui crates have MSRV 1.95. Through `eframe`, the
tree gains `arboard`/`clipboard-win` (BSL-1.0, allowlisted, see LEGAL.md)
for copy and paste. Wayland and X11 libraries are loaded at run time
(`dlopen`), so no system development packages are needed to build.
`egui_kittest` was not in the plan; it is egui's own test harness and
replaces a hand-written AccessKit walker.

M9 (2026-10-06): `librqbit` (chosen in ADR-0012; `cratetorrent` is
unmaintained) brings its own `axum` and `reqwest`. Its default features
are off; `rust-tls` keeps TLS on rustls. The server uses `hyper` directly
because it serves one route and needs exact control of range responses;
`axum` would add routing we do not use. Through `librqbit-core` the tree
gains `directories` 6.0.0 and with it `option-ext` (MPL-2.0), which has a
crate-scoped exception in `.config/deny.toml` (LEGAL.md).

2026-10-07: `librqbit` comes from the owner's fork (`[patch.crates-io]`
in the root `Cargo.toml`, a git source allowed in `.config/deny.toml`),
pinned to a commit of branch `fix/urgent-piece-helpers`: upstream `main`
plus our streaming patch (ADR-0016). To update: rebase the branch on
upstream, push, and change `rev`. Dropping the fork once upstream
releases the patch is the goal.

UI polish (2026-10-07): not crates, but bundled assets. `cineo-desktop`
embeds Inter 4.1 (Regular and SemiBold) and DM Serif Display (Regular),
about 910 KB together, all OFL-1.1 ([LEGAL.md](LEGAL.md#bundled-assets)), with
`include_bytes!`. Icons come from `iconflow` 2.1.0 (crates.io,
2026-10-07: MIT, MSRV 1.92, no runtime dependencies) with only the Tabler
pack enabled, which embeds Tabler's regular and filled icon fonts (about
1.5 MB). The other 13 packs stay off. egui's `default_fonts` stay enabled as the fallback for symbols and emoji.

### Planned (researched, not added)

Researched with versions as of 2026-10-06. Rows for crates that were
added since are in the table above. One open question from that research:
`reqwest`'s `rustls` feature uses aws-lc-rs, whose C build on Windows CI is
**UNKNOWN** (fallback: `rustls-no-provider` + ring).

| Concern | Choice | Version | License | MSRV | Added in | Why / alternatives considered |
|---------|--------|---------|---------|------|----------|-------------------------------|
| Secrets | `keyring` | 4.2.0 | MIT/Apache | 1.88 | M10 (NZB server credentials) | OS keychains. Not needed until there are credentials; cloud sync is not planned |
| Property tests (dev) | `proptest` | 1.11.0 | MIT/Apache | 1.85 | When first useful | URL round-trips, parser robustness |
| Snapshots (dev) | `insta` | 1.49.0 | Apache | 1.66 | Only if needed | Large CLI outputs |
| Fuzzing | `cargo-fuzz` / `libfuzzer-sys` | 0.13.2 / 0.4.13 | MIT/Apache (+NCSA) | — | After M2 | Parser fuzz targets |

### Deliberately not used

| Crate | Reason |
|-------|--------|
| `libmpv2` (LGPL-2.1) | Would compile LGPL code into the binary and link libmpv at build time; own bindings loaded at runtime instead (ADR-0014) |
| `slint` | GPL or proprietary licensing |
| `serde_with` | The ADR-0003 lenient parser needs warnings, which serde adapters cannot emit |
| `chrono` / `jiff` / `time` | Not needed until dates are modelled (M2 `released`). Decide then; `jiff` is the current front-runner |
| `directories` 6.0.0 | Pulls in `option-ext` (MPL-2.0, not in the `.config/deny.toml` allowlist) via `dirs-sys`. `etcetera` gives the same paths with only `cfg-if`/`windows-sys` (2026-10-06) |
| `sqlx` / `redb` | `sqlx`: async with compile-time checks, heavier than needed. `redb`: key-value, no queries. `rusqlite` chosen in M4 |
| `async-trait` | Native async fn in traits is enough; effects are data (ADR-0001) |
| `figment` / config crates | No configuration file yet |
| OpenSSL (`native-tls`) | rustls only; banned in `.config/deny.toml` |

### Tools (not crate dependencies)

| Tool | Version | Use |
|------|---------|-----|
| cargo-nextest | 0.9.146 | CI test runner |
| cargo-deny | 0.20.2 | Licenses, advisories, bans, sources |
| zizmor | via action v0.6.4 | GitHub Actions security lint |

## Releasing

There are no releases yet. The pipeline exists so the first release is
boring.

- **Versioning:** SemVer. `0.x` until v1.0, and anything may change in
  `0.x`. The workspace version in the root `Cargo.toml` is the single source.
- **Changelog:** [Keep a Changelog](https://keepachangelog.com) format in
  `CHANGELOG.md`. Every user-visible change adds a line under *Unreleased*
  in the same PR.
- **Tags:** `vMAJOR.MINOR.PATCH`, with an optional pre-release suffix
  (`v0.1.0-alpha.1`). Tags containing `-` become GitHub pre-releases.

### Steps

1. On `main` with green CI: move the *Unreleased* entries to a new version
   section and bump `version` in `Cargo.toml`.
2. Commit with `chore(release): vX.Y.Z`.
3. Tag `vX.Y.Z` and push the tag.
4. `.github/workflows/release.yml`:
   - tests and builds `cineo` for Linux x86_64, Windows x86_64 and macOS
     arm64,
   - packages each build with the README and licenses,
   - attests build provenance,
   - writes `SHA256SUMS`,
   - creates a **draft** release.
5. Review the draft (download, check a checksum, run
   `gh attestation verify <file> --repo <owner>/cineo`), then publish.

### Not yet decided (M7)

- Code signing (Windows Authenticode, macOS notarization) and installer
  formats.
- Bundling libmpv (license review: [LEGAL.md](LEGAL.md)).
- Reproducible-build verification beyond `--locked` + a pinned toolchain.

## Pull requests

- Run `scripts/check.sh` before opening one.
- **AI assistance** must be disclosed in the PR description. You are
  responsible for every line you submit.

By contributing, you agree that your contributions are licensed under the
project's MIT license.

By contributing, you agree that your contributions are licensed under the
project's MIT license.
