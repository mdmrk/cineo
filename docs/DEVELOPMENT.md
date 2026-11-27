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

1. Install [rustup](https://rustup.rs). It installs the Rust version that
   `rust-toolchain.toml` pins, with rustfmt and clippy, the first time you
   run `cargo` (ADR-0036).
2. Optional tools, which CI always runs:
   ```sh
   cargo install --locked cargo-nextest cargo-deny taplo-cli typos-cli cargo-machete
   ```
   `shellcheck` and `shfmt` come from your package manager. `scripts/fmt.sh`
   and `scripts/check.sh` skip a missing tool locally
   (`check.sh` runs `cargo test` without nextest) and fail on it in CI.
3. libmpv (mpv 0.35 or newer) to play anything; the app loads it at
   runtime (ADR-0014). On Windows, put a `libmpv-2.dll` next to the
   executable, or build with `--features bundled-libmpv` and
   `CINEO_LIBMPV_DLL_GZ` set to the gzip-compressed DLL
   (`scripts/fetch-libmpv.sh windows <dir>` downloads the pinned one and writes
   `<dir>/libmpv-2.dll.gz`). On macOS, `brew install mpv` for
   `cargo run`; `scripts/fetch-libmpv.sh macos target/libmpv-macos` then
   `scripts/package.sh macos` (after release builds for
   `aarch64-apple-darwin` and `x86_64-apple-darwin`) makes the `.dmg`.
4. To check the Windows build from Linux:
   `rustup target add x86_64-pc-windows-gnu`, plus the `mingw-w64-gcc`
   package for the C dependencies, then
   `cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings`.
   Run the tests with `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine`.
   The engine tests need an absolute Windows path, which a cross-compiled
   `CARGO_TARGET_TMPDIR` (`/home/…`) is not, so they fail under wine only.
   CI tests on `windows-latest` (MSVC).
5. To build the Android app, see [Android](#android).

If you build with a distro-packaged Rust instead of rustup, make sure it is
the version `rust-toolchain.toml` pins or newer. Some distro builds expect a cross linker name; if
linking fails with `linker 'x86_64-linux-gnu-gcc' not found`, set
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc` in your shell. Do not commit
that setting.

### Android

The Android app (ADR-0028) needs:
- the Rust target `aarch64-linux-android` (`scripts/install-rust.sh
  aarch64-linux-android`);
- cargo-ndk at the version `CARGO_NDK_VERSION` in `scripts/android.env`
  pins (`cargo install --locked cargo-ndk@<version>`);
- JDK 21;
- the Android SDK with its licenses accepted (`sdkmanager --licenses`) and
  the NDK that `NDK_VERSION` in `scripts/libmpv-android.env` names. Point
  `ANDROID_HOME` at the SDK, or write `sdk.dir=…` to
  `android/local.properties`.

**Debug build.** `cd android && ./gradlew assembleDebug` builds
`app/build/outputs/apk/debug/app-debug.apk`; `adb install -r` puts it on a
device. Gradle's `cargoNdk` task builds `cineo-android` for `arm64-v8a` at
API level 26, and `uniffiBindgen` writes the Kotlin bindings from that
library into `app/build/generated/rust/kotlin` with the `uniffi-bindgen`
of `cineo-uniffi-bindgen` (ADR-0038). The Android build keeps
symbols (`CARGO_PROFILE_RELEASE_STRIP=debuginfo`) because `uniffi-bindgen`
reads its metadata from them; AGP strips the copy in the APK. It also
unwinds on panic (`CARGO_PROFILE_RELEASE_PANIC=unwind`), unlike the desktop
release, so a panicking thread is reported (`the shell thread panicked`)
and UniFFI turns a panic in a call into a Kotlin exception instead of
closing the app.
`./gradlew testDebugUnitTest` runs the Kotlin unit tests in `app/src/test/`
on the JVM, and `./gradlew lintDebug` runs Android Lint, which fails on
errors and reports warnings in `app/build/reports/`. CI runs
`cargo ndk … clippy -p cineo-android`, the Gradle build, those tests and
Lint (job `check (android)`).

**libmpv.** `ANDROID_NDK_HOME=… scripts/build-libmpv-android.sh` needs
`python3` (with `venv`), `perl`, `make`, `ninja`, `pkg-config`, `git`,
`curl` and `strings` (binutils) on the host. It downloads the pinned sources and checks their SHA-256, installs
the pinned meson in a venv (its wheel checked by SHA-256), cross-compiles OpenSSL, dav1d, freetype, fribidi,
harfbuzz, libass, FFmpeg, libplacebo and mpv for `arm64-v8a` at API level
26 in the fixed `/tmp/cineo-libmpv-android` (sources, build, prefix and
meson; fixed so that the paths the libraries record are the same on every
machine), and writes `target/libmpv-android/out/` (`lib/`, `licenses/`,
`SOURCES` and `libmpv.md` with each `.so`'s SHA-256). `libmpv.md` lists the pinned sources
with their URLs and hashes; no source archive is written (ADR-0029,
amended 2026-11-05). It stops if FFmpeg was configured with
`--enable-gpl`, `--enable-version3` or `--enable-nonfree`, if mpv has GPL
code, or if a library needs one that is neither shipped nor part of
Android. Gradle copies `out/lib/` into the APK, and `out/licenses/` and
`libmpv.md` into `assets/licenses/`. A debug build without them only
warns; a release build fails.

**Release APK.** Set `CINEO_KEYSTORE` (the `.jks` file),
`CINEO_KEYSTORE_PASSWORD`, `CINEO_KEY_ALIAS` and `CINEO_KEY_PASSWORD`, then
run `scripts/release-android.sh`, the same steps as CI's `build (android)`.
It refuses to build without the four variables or with a cargo-ndk other
than `CARGO_NDK_VERSION`, finds the SDK through
`ANDROID_HOME` or `android/local.properties`, rebuilds libmpv only when the
script or its `.env` changed since the last build
(`target/libmpv-android/pins`), runs `./gradlew assembleRelease` and
`scripts/package.sh android`, which checks the APK, and leaves
`dist/cineo-arm64-android.apk`. Without `CINEO_KEYSTORE`,
`./gradlew assembleRelease` writes an unsigned `app-release-unsigned.apk`.
The `versionName` and `versionCode` come from `Cargo.toml` ([Releasing](#releasing)).
Release builds run R8 with the keep rules in
`android/app/proguard-rules.pro` for JNA, the UniFFI bindings and
rustls-platform-verifier, and store the `.so` files compressed. AGP keeps
the libmpv and FFmpeg `.so` files unstripped, so they match the hashes in
`libmpv.md`. `android/rustls-platform-verifier` is the verifier's Kotlin
half, copied unchanged from upstream (`UPSTREAM` names the commit); Gradle
fails if its version differs from `rustls-platform-verifier-android` in
`Cargo.lock` (ADR-0037).

## Everyday commands

| Task                 | Command                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------- |
| Everything CI checks | `scripts/check.sh` (`--quick` stops after the tests: no taplo, shellcheck, typos, cargo-machete, doc references, release scripts or deny) |
| Run the desktop app  | `cargo run -- -v` (`--data-dir`, `--cache-dir`, `--portable`, `--allow-private-network`) |
| Format everything    | `scripts/fmt.sh` (Rust, TOML, shell; `--check` only verifies)                               |
| Lint                 | `cargo clippy --workspace --all-targets --locked -- -D warnings`                            |
| Test                 | `cargo nextest run --workspace` or `cargo test --workspace`                                 |
| One test             | `cargo test -p cineo-core some_name`                                                        |
| Dependency policy    | `cargo deny --config .config/deny.toml check`                                               |
| Translations         | `scripts/update-translations.sh` (GNU gettext 0.24+; refreshes `cineo.pot` and merges it into every `.po`; `--check` only verifies) |

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
- Public API: keep items `pub(crate)` unless another crate needs them.
- Comments: none, including doc comments. Names, types and tests carry the
  meaning; lint exceptions carry their `reason = "…"`.
- Domain types enforce invariants in constructors (`new`/`parse` returning
  `Option`/`Result`). Do not pass raw `String`s for ids, types or URLs
  across modules.
- Async only in IO crates and shells (tokio). `cineo-core` stays sync and
  pure.
- Tests: behavior-named, fixtures from `tests/`, no network, no
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
- `#[allow(...)]` needs a `reason = "…"`. `#[expect(...)]` is
  preferred when the lint should fire.
- `unsafe_code = "deny"` in `[workspace.lints]`, and `#![forbid(unsafe_code)]`
  in every crate root without FFI; the modules that allow it and the
  exception process are in [SECURITY.md](SECURITY.md).

### Rust version (ADR-0036)

`rust-toolchain.toml` pins one stable release, and CI installs that one with
`scripts/install-rust.sh [target…]` (`rustup toolchain install`, then the
targets; it does not change your default toolchain).
To move to a new release, change `channel`, fix the new lints and commit
both as `build: use Rust X.Y.Z`. The pin keeps CI's Rust cache warm:
`Swatinem/rust-cache` hashes the `rustc` version into its key, so an
unpinned `stable` started every job cold with each Rust release.

## Testing

**Principle:** a behavior is not supported until a test proves it. Bug fixes
start with a failing regression test. Add test infrastructure only when the
first test needs it.

### Pyramid

```
            ┌───────────────┐  few: headless UI tests
          ┌─┴───────────────┴─┐  some: integration (IO crate vs local mock / temp DB / real libmpv, opt-in)
      ┌───┴───────────────────┴───┐  many: unit + protocol fixtures (pure core, fast)
```

| Layer               | Where                                        | Tools                                                                                                                              | Runs in CI                         |
| ------------------- | -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- |
| Unit                | `#[cfg(test)] mod tests` next to the code    | std test harness                                                                                                                   | always                             |
| Protocol fixtures   | `crates/cineo-core/tests/` + `tests/addons/` | JSON fixtures                                                                                                                      | always                             |
| Network integration | `crates/cineo-net/tests/`                    | local mock HTTP server (`wiremock`)                                                                                                | always                             |
| Persistence         | `crates/cineo-store/tests/`                  | temp-dir SQLite, fixture DBs per schema version                                                                                    | always                             |
| Player              | `crates/cineo-player/src/` (unit tests)      | pure command, track-list and event-mapping tests; real libmpv tests marked `#[ignore]` (`cargo test -p cineo-player -- --ignored`) | pure: always; real: CI (Linux)      |
| Android UI logic    | `android/app/src/test/`                      | JUnit 4 on the JVM (`./gradlew testDebugUnitTest`)                                                                                 | always (check (android))           |

### Rules

- **Name tests after the behavior**:
  `quirky_streams_drop_bad_sources_and_unsafe_headers`, not `test_parse_3`.
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

Commands are under [Everyday commands](#everyday-commands).

### Per-subsystem strategy

| Subsystem          | Strategy                                                                                                                             |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| Protocol parsing   | Fixtures for basic, quirks and invalid cases; every lenient rule has a fixture case                                                  |
| URL building       | Table tests including the docs' example, reserved chars, unicode, prefix and query transport URLs                                    |
| Filtering/planning | Fixture manifests × request matrix                                                                                                   |
| Network policy     | Pure IP/URL classification tables + mock-server tests (blocked loopback, redirects, size, gzip bomb, timeout, no URL leak in errors) |
| Aggregation        | Reducer tests: effects emitted, partial failure, stale responses dropped                                                             |
| Player (ADR-0014)  | Commands built from typed values; event mapping tables; real-libmpv playback `#[ignore]`; controls via kittest                       |
| Persistence        | Round-trip; migration from each historical schema fixture; corruption handling                                                       |
| GUI                | Core-level tests carry the logic; `egui_kittest` tests drive the views headlessly                                                    |

## Debugging

Failures must be observable without a debugger. This page says what each
subsystem logs and how to diagnose it.

### Logging

- `tracing` everywhere. Libraries never print. Only shells write to
  stdout/stderr.
- GUI: `cineo` logs to stderr. `-v` gives debug logs for Cineo
  crates, `-vv` gives trace for everything. `RUST_LOG` overrides both, for
  example `RUST_LOG=cineo_net=trace`. libmpv's own messages use the target `libmpv`: its
  errors are always shown, its warnings (about the media, some on every
  frame) only with `-v`. Each run also writes `<data dir>/cineo.log` and
  keeps the run before it as `cineo.previous.log`. By default that file
  holds Cineo's warnings and other crates' errors only; `-v`, `-vv` and
  `RUST_LOG` widen it too. There is no diagnostics view
  ([TECHNICAL_DEBT.md](TECHNICAL_DEBT.md)).

#### Span conventions

| Span            | Fields                                                                               | Emitted by  |
| --------------- | ------------------------------------------------------------------------------------ | ----------- |
| `addon_request` | `req` (process-unique id), `addon` (origin only), `resource` (encoded resource path) | `cineo-net` |
| `image_request` | `req`, `origin`                                                                      | `cineo-net` |

**Never log** full transport URLs, query strings of addon URLs, stream URLs
with tokens, or headers from `proxyHeaders`. Log the origin and path instead.

### Diagnosing

#### Addon failures

1. Install the addon in `cineo -v`. Every parse **warning** is
   logged as `addon data problem` (field location + reason) inside the
   `addon_request` span, on stderr and in `cineo.log`.
2. Read the error type. It is one of:
   - `blocked by network policy` (private address, redirect downgrade, too
     many redirects)
   - `timed out`
   - `HTTP <status>`
   - `exceeds the N-byte limit`
   - `invalid manifest: …` (the field is named)
3. If another client accepts the addon and Cineo does not, compare against
   [ADDON_PROTOCOL.md](ADDON_PROTOCOL.md). Then file an "Addon
   compatibility" issue with those log lines.

#### Catalog / metadata failures

- Undeclared catalogs and invalid extras are never requested; the
  `addon_request` span shows which catalogs were asked for.
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

- Installed addons and the library live in one SQLite database, by default
  `$XDG_STATE_HOME/cineo/cineo.db` (`~/.local/state/cineo/cineo.db`);
  `--data-dir <dir>` uses another directory, and `--portable` or a
  `portable.txt` file in the working directory uses the working directory,
  with the cache in its `cache` folder. The database uses write-ahead
  logging, so while Cineo runs recent writes live in `cineo.db-wal`; copy
  `cineo.db` only after closing it. Addon URLs in it can contain your addon
  configuration; do not share it publicly.
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

The UI tests (`crates/cineo-desktop/tests/ui/`) check rendering and the
actions clicks produce. This script covers what they cannot: real windows,
real addons, libmpv, and restarts. Run it before a release that changes the
GUI, player or engine, and record the date, desktop session (Wayland or X11)
and result in the PR.

### Setup

- Build: `cargo build -p cineo-desktop --release`. libmpv (mpv 0.35 or
  newer) must be installed for playback.
  Release builds strip symbols and abort on panic (`panic = "abort"`), so a
  panic anywhere, including a background task, closes the app and its
  backtrace shows addresses only; reproduce with a debug build to get
  function names. Each panic also adds a line to `<data dir>/panic.log` with
  its location, without its message.
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

### Steps

| #   | Do                                                                                 | Expect                                                                                       |
| --- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| 1   | Start `cineo --data-dir <tmp> -v`                                          | Board shows Cinemeta rows with posters                                                       |
| 2   | Addons                                                                             | Cinemeta is the only installed addon; Discover addons shows the Official and Community lists |
| 3   | Install the stream addon (`http://127.0.0.1:8000/manifest.json` for the local one) | It appears second in the list; Move up/down reorders                                         |
| 4   | Search "Night of the Living Dead" → open the 1968 film                             | Detail page: backdrop, poster, facts, description                                            |
| 5   | Streams list                                                                       | One group per stream addon; the local stream has an enabled Play                             |
| 6   | Play                                                                               | The player opens in the window and plays. Total time from step 2: under a minute             |
| 7   | Seek to ~10 minutes, wait 10 s, press Back                                         | Board shows "Continue watching" with a progress bar                                          |
| 8   | Quit Cineo, start it again with the same `--data-dir`                              | Addons (in order) and Continue watching are back                                             |
| 9   | Open the item from Continue watching → Play                                        | Playback resumes at ~10 minutes                                                              |
| 10  | Discover → pick "Popular — Cinemeta", a genre, "Load more"                         | Items change with the genre; more items append                                               |
| 11  | Open a series → choose a season → an episode                                       | Episode list for that season; streams load for the episode                                   |
| 12  | Torrent streams                                                                    | See [Torrents](#torrents)                                                                    |
| 13  | Library → Remove the item                                                          | It disappears, and stays gone after a restart                                                |
| 14  | Install `http://127.0.0.1:9/manifest.json` without `--allow-private-network`       | Error mentions the network policy; nothing crashes                                           |
| 15  | Addons → Discover addons → Official, then Community → Install one                  | Lists load; installed addons show "Installed"; the new addon joins the installed list        |

Also watch the `-v` log: no panics, and no full addon or stream URLs (only
origins and resource paths).

### Torrents

Use a torrent of content you may share (for example a public-domain film
from the Internet Archive; each item has a torrent whose info hash is on
its page). Add a stream to the local addon above, in front of the
`Local` entry:

```json
{ "name": "Torrent", "infoHash": "<40 hex characters>", "fileIdx": 0 }
```

Use a fresh `--data-dir` and `--cache-dir` so the P2P notice appears.

| #   | Do                                                               | Expect                                                                                                                                                      |
| --- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T1  | Open the film; Play the torrent stream                           | The "Peer-to-peer streaming" notice; nothing appears in the cache directory yet                                                                             |
| T2  | Cancel                                                           | No playback, no engine log lines (`-v`)                                                                                                                     |
| T3  | Play again → Accept and play                                     | The player opens at once with Back and the film's logo (or, without one, its title) pulsing in the middle; the video starts once the engine serves the file |
| T4  | Seek forward and back                                            | Playback resumes within a few seconds                                                                                                                       |
| T5  | Back (or Esc)                                                    | The detail page returns; the log shows the torrent stopped. Back while still connecting cancels the torrent too                                             |
| T6  | Play it again                                                    | No notice this time. While it plays, the cache directory holds its `<info hash>` folder with only `<piece index>` files (no torrent file names)             |
| T7  | Settings → untick "Show and play torrent streams"; open the film | The torrent stream is gone; "1 torrent stream hidden…" is shown                                                                                             |
| T8  | Restart with the same directories                                | The setting is kept                                                                                                                                         |
| T9  | Back, then look in the cache directory                           | Only `dht.json`: the torrent's data was deleted when it stopped                                                                                             |

## Dependencies

1. Check that it is really needed. Prefer std or an existing dependency.
2. Research it: current version, license, maintenance, MSRV, alternatives.
3. Add it to `[workspace.dependencies]` with minimal features. Crates use
   `dep.workspace = true`.
4. Add a row to the _In use_ table below.
5. `cargo deny --config .config/deny.toml check` must pass. A new license needs a
   [LEGAL.md](LEGAL.md) note.

Updates are manual (`cargo update`, then `scripts/check.sh`); there is
no Dependabot, so no bot branches.

Every direct dependency gets a row **when it is added**, with the version
in `Cargo.toml`; `scripts/check-docs.sh` fails otherwise. Research the
current version when adding one.

### In use

| Crate                                                                                  | Version                                                                                                                                | Purpose                                                                                                                                         | License                              |
| -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| `percent-encoding`                                                                     | 2.3.2                                                                                                                                  | `encodeURIComponent` path encoding (core)                                                                                                       | MIT/Apache                           |
| `semver`                                                                               | 1.0.28                                                                                                                                 | Manifest `version` (core), release versions (core, net, desktop)                                                                                | MIT/Apache                           |
| `serde_json`                                                                           | 1.0.154                                                                                                                                | JSON documents (core, net)                                                                                                                      | MIT/Apache                           |
| `thiserror`                                                                            | 2.0.21                                                                                                                                 | Library errors                                                                                                                                  | MIT/Apache                           |
| `url`                                                                                  | 2.5.9                                                                                                                                  | URLs (core, net)                                                                                                                                | MIT/Apache                           |
| `reqwest` (`rustls`, `gzip`, `brotli`, `http2`; no default features)                   | 0.13.5                                                                                                                                 | Addon HTTP client (net)                                                                                                                         | MIT/Apache                           |
| `tokio`                                                                                | 1.54.1                                                                                                                                 | Async runtime (net, stream, player, shells)                                                                                                      | MIT                                  |
| `tracing` / `tracing-subscriber`                                                       | 0.1.44 / 0.3.23                                                                                                                        | Logging                                                                                                                                         | MIT                                  |
| `anyhow`                                                                               | 1.0.105                                                                                                                                | Shell errors (desktop, stream)                                                                                                                  | MIT/Apache                           |
| `clap`                                                                                 | 4.6.9                                                                                                                                  | Command-line flags (desktop)                                                                                                                    | MIT/Apache                           |
| `wiremock` (dev)                                                                       | 0.6.5                                                                                                                                  | Mock addon servers in tests                                                                                                                     | MIT/Apache                           |
| `tempfile` (dev)                                                                       | 3.27.0                                                                                                                                 | Temporary directories removed after each test                                                                                                   | MIT/Apache                           |
| `rusqlite` (`bundled`, no default features)                                            | 0.43.0                                                                                                                                 | SQLite database (store)                                                                                                                         | MIT; bundled SQLite is public domain |
| `fastframe-dirs` (git, commit; not on crates.io) | `a7dc9bd` (0.17.1 + `main`) | Per-user data and cache folders, through `directories` (desktop, ADR-0034) | MIT |
| `eframe` (`glow`, `default_fonts`, `clipboard`, `x11`, `wayland`, `accesskit`, `links`; no default features) | 0.37.0 (ADR-0042)                                                                                                                                 | Desktop window and egui (desktop, ADR-0011)                                                                                                     | MIT/Apache                           |
| `iconflow` (`pack-tabler` only; no default features)                                   | 2.1.0                                                                                                                                  | Tabler icon font and codepoints for the desktop UI (desktop)                                                                                    | MIT; Tabler font MIT                 |
| `image` (`jpeg`, `png`, `webp` only)                                                   | 0.25.10                                                                                                                                | Image header checks and decoding (desktop)                                                                                                      | MIT/Apache                           |
| `winresource` (build dependency; no default features)                                  | 0.1.34                                                                                                                                 | Puts the Cineo icon (`assets/brand/icon.ico`) and the version into the Windows executable (desktop); runs only for Windows targets              | MIT                                  |
| `flate2`                                                                               | 1.1.11                                                                                                                                 | Decompresses the bundled `libmpv-2.dll` (desktop, ADR-0027); already in the graph through librqbit                                              | MIT/Apache                           |
| `minisign-verify`                                                                      | 0.3.0                                                                                                                                  | Verifies the release's `SHA256SUMS.minisig` before an in-app update (desktop, Windows and Linux, ADR-0040, ADR-0041); no dependencies | MIT |
| `aws-lc-rs`                                                                            | 1.19.0                                                                                                                                 | SHA-256 of downloaded update files (desktop, ADR-0040, ADR-0041); sync key derivation, encryption and randomness (net, ADR-0043); already in the graph through rustls and librqbit | ISC AND (Apache-2.0 OR ISC) |
| `data-encoding`                                                                        | 2.11.1                                                                                                                                 | Base32 sync codes and base64url sync tokens and blobs (net, ADR-0043); already in the graph | MIT |
| `zeroize`                                                                              | 1.9.2                                                                                                                                  | Wipes the derived sync token from memory (net, ADR-0043); already in the graph | Apache-2.0 OR MIT |
| `fast_image_resize` (default features)                                                 | 6.2.0                                                                                                                                  | SIMD downscaling of posters and backdrops (desktop)                                                                                             | MIT/Apache                           |
| `egui_kittest` (dev, no default features)                                              | 0.37.0 (ADR-0042)                                                                                                                                 | Headless UI tests via AccessKit (desktop)                                                                                                       | MIT/Apache                           |
| `librqbit` (`rust-tls`, `disable-upload`, no default features)                         | 9.1.0 + our patch, from branch `main` of the [fork](https://github.com/mdmrk/rqbit) (upstream `main` merged with `feat/stream-window`) | BitTorrent session (stream, ADR-0012)                                                                                                           | Apache-2.0                           |
| `zbus` (`async-io`; no default features; Linux only)                      | 5.19.0, the version `accesskit_unix` and fastframe use, so only one builds                                              | Keeps the screen awake during playback over the session D-Bus (desktop)                                                                         | MIT                                  |
| `fastframe-now-playing` (git, commit; not on crates.io)                                   | `a7dc9bd` (0.17.1 + `main`)                                                                                                                                 | Media keys and the desktop's media controls: MPRIS through `mpris-server` 0.10.0 on Linux, SMTC through `souvlaki` 0.8.3 on Windows, Now Playing through it on macOS (desktop, ADR-0031, ADR-0035) | MIT (`mpris-server`: MPL-2.0)        |
| `fastframe-instance` (git, commit; not on crates.io) | `a7dc9bd` (0.17.1 + `main`) | Hands a link from a new launch to the running window (desktop, ADR-0032) | MIT |
| `fastframe-file` (git, commit; not on crates.io) | `a7dc9bd` (0.17.1 + `main`) | Replaces the bundled libmpv, the link registration entry and the torrent blocklist without leaving a half-written file (desktop, stream, ADR-0032) | MIT |
| `fastframe-power` (git, commit; not on crates.io) | `a7dc9bd` (0.17.1 + `main`) | Blocks idle system sleep during playback and pauses before the computer sleeps (desktop, ADR-0032); the screen stays on through `zbus` and `keepawake` | MIT |
| `fastframe-log` (git, commit; no default features) | `a7dc9bd` (0.17.1 + `main`) | Records panics in `panic.log` without their message (desktop, ADR-0032) | MIT |
| `fastframe-emoji` (git, commit; not on crates.io) | `a7dc9bd` (0.17.1 + `main`) | Colour emoji in egui text (desktop, ADR-0033) | MIT |
| `fastframe-fonts` (git, commit; no default features) | `a7dc9bd` (0.17.1 + `main`) | Installed fonts as fallbacks for other scripts (desktop, ADR-0033) | MIT |
| `fastframe-text` (`watch`; git, commit; not on crates.io) | `a7dc9bd` (0.17.1 + `main`) | Follows the desktop's font rendering and text size, live (desktop, ADR-0033) | MIT |
| `fastframe-scroll` (git, commit; not on crates.io) | `a7dc9bd` (0.17.1 + `main`) | Touchpad scrolling with glide on Linux (desktop, ADR-0033) | MIT |
| `fastframe-graphics` (git, commit; not on crates.io) | `a7dc9bd` (0.17.1 + `main`) | Explains in a dialog when no OpenGL window can open (desktop, ADR-0033) | MIT |
| `keepawake` (Windows and macOS)                                                        | 0.6.1                                                                                                                                  | Keeps the screen awake during playback: `SetThreadExecutionState` on Windows (ADR-0027), IOKit assertions on macOS (ADR-0035)                                         | MIT                                  |
| `hyper` (`server`, `http1`) / `hyper-util` (`tokio`) / `http-body-util`                | 1.13.0 / 0.1.21 / 0.1.5                                                                                                                | Loopback HTTP server for the player (stream)                                                                                                    | MIT                                  |
| `bytes`                                                                                | 1.12.2                                                                                                                                 | Response bodies (stream)                                                                                                                        | MIT                                  |
| `futures-util` (no default features)                                                   | 0.3.34                                                                                                                                 | Stream adapters for bodies (stream)                                                                                                             | MIT/Apache                           |
| `tokio-util` (`io`)                                                                    | 0.7.20                                                                                                                                 | `ReaderStream` from a torrent file reader (stream)                                                                                              | MIT                                  |
| `getrandom`                                                                            | 0.4.3                                                                                                                                  | Per-session path token and proxy password (stream)                                                                                              | MIT/Apache                           |
| `fastframe-i18n` (git, commit; `build` feature at build time only) | `a7dc9bd` (0.17.1 + `main`) | Compiles the gettext UI catalogs in `build.rs` and looks messages up, and picks the default UI language from the OS (desktop, ADR-0034) | MIT |
| `raw-window-handle`                                                                    | 0.6.2                                                                                                                                  | The window's Wayland or Xlib display for mpv's hardware-decoding interop, and the HWND for SMTC on Windows (desktop); already in the tree through eframe | MIT/Apache/Zlib                      |
| `libloading`                                                                           | 0.9.1                                                                                                                                  | Loads libmpv at runtime for embedded playback (player, ADR-0014). glutin still pulls in 0.8.9 on Linux, so the tree has both; 0.8.9 words a load error differently and fails `a_load_error_keeps_the_system_reason` (2026-11-17) | ISC                                  |
| `uniffi` (no default features; `cli` and `cargo-metadata` only in `cineo-uniffi-bindgen`) | 0.32.2                                                                                                                                 | Kotlin bindings for the Android shell (android, ADR-0028)                                                                                        | MPL-2.0 (LEGAL.md)                   |
| `jni` (no default features; Android only)                                              | 0.22.4                                                                                                                                 | The one JNI entry point that hands the app `Context` to the TLS verifier (android)                                                              | MIT/Apache                           |
| `rustls-platform-verifier` (Android only)                                              | 0.7.1, already in the tree through reqwest                                                                                             | `android::init_with_env` for Android's certificate verifier (android)                                                                           | MIT/Apache                           |
| `android_logger` (no default features; Android only) | 0.15.1 | Sends logs to logcat (android) | MIT/Apache |
| `log` (Android only) | 0.4.35 | Maximum level for `android_logger`; `tracing` forwards to it through its `log` feature (android) | MIT/Apache |

`tokio` features `io-util` and `sync` serve the mpv player; `process` was
dropped with the external mpv player (2026-10-07). The player uses
`serde_json` for mpv's track list.

`rusqlite` drops its default features (`cache`, and an FFI backend used only
on wasm). `bundled` compiles SQLite from source, so no system library is
needed.

egui (2026-10-06): the egui crates have MSRV 1.95. Through `eframe`, the
tree gains `arboard` for copy and paste. Wayland and X11 libraries are loaded at run time
(`dlopen`), so no system development packages are needed to build.
`egui_kittest` was not in the plan; it is egui's own test harness and
replaces a hand-written AccessKit walker.

librqbit (2026-10-06, ADR-0012; `cratetorrent` is
unmaintained) brings its own `axum` and `reqwest`. Its default features
are off; `rust-tls` keeps TLS on rustls. The server uses `hyper` directly
because it serves one route and needs exact control of range responses;
`axum` would add routing we do not use. Through `librqbit-core` the tree
gains `directories` 6.0.0 and with it `option-ext` (MPL-2.0), which has a
crate-scoped exception in `.config/deny.toml` (LEGAL.md).

2026-10-07: `librqbit`'s `disable-upload` feature is on for the "Upload to
other peers" setting. It adds no dependency (`disable-upload = []`); it
compiles the `SessionOptions::disable_upload` switch, which stops
advertising pieces and drops a peer that asks for one anyway.

`librqbit` comes from branch `main` of the owner's fork (`[patch.crates-io]`
in the root `Cargo.toml`, a git source allowed in `.config/deny.toml`,
ADR-0016): upstream `main` (9.1.0) plus our streaming patches. Those add the
urgent-piece helpers, the stream window (ADR-0019), the lookahead setter
(ADR-0023), `SessionOptions.http_proxy_url` and filtering peer addresses by
the blocklist before metadata is asked for (ADR-0026). Since 2026-11-15
(`8981a054`, with upstream up to 2026-11-12) they also give up, after a
seek, the in-flight pieces no stream wants and cancel their requests, and
take a chunk out of the upload queue when its peer sends `cancel`. Since
2026-11-19 (`984d0a4f`) closing a stream gives them up too. To
update: merge
upstream `main` into the fork's `main`, push, and run
`cargo update -p librqbit`. Dropping the fork once upstream releases the
patch is the goal.

UI polish (2026-10-07): not crates, but bundled assets. `cineo-desktop`
embeds Inter 4.1 (Regular and SemiBold) and DM Serif Display (Regular, used only by the subtitle preview),
about 910 KB together, all OFL-1.1 ([LEGAL.md](LEGAL.md#bundled-assets)), with
`include_bytes!`. Icons come from `iconflow` 2.1.0 (crates.io,
2026-10-07: MIT, MSRV 1.92, no runtime dependencies) with only the Tabler
pack enabled, which embeds Tabler's regular and filled icon fonts (about
1.5 MB). The other 13 packs stay off. The logo (656×618) and window icon (256×256) are PNGs in `assets/brand/`, decoded with the `image` crate already in use. The loading animation's source is `assets/brand/loading.svg` (SMIL); egui cannot play SVG animation, so it is rasterized at 2× into layers in `assets/brand/loading/` (static parts, reel, each moustache half, and a 5×2 sheet of the film strip's 10 frames) that `brand::paint_loading` rotates and steps in code. All brand PNGs come to about 130 KB. egui's `default_fonts` stay enabled as the fallback for symbols and emoji.

2026-10-09: `fast_image_resize` 6.1.0 (crates.io: MIT/Apache, MSRV 1.87,
released 2026-07-21, about 21 M downloads) replaces `image`'s Triangle
resize for network images. Measured on a 580×859 JPEG poster scaled to
card size (release build): decode plus resize went from 6.96 ms to
3.51 ms; the resize itself from about 6 ms to 0.6 ms. Its `image` and
`rayon` features stay off. Its dependencies (`cfg-if`, `num-traits`,
`thiserror`, `document-features`) were already in the tree, so it is the
only new crate in `Cargo.lock`. RGBA images are resized with premultiplied
alpha (`colour_and_transparency_survive_downscaling`).

### Deliberately not used

| Crate                      | Reason                                                                                                                                 |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `libmpv2` (LGPL-2.1)       | Would compile LGPL code into the binary and link libmpv at build time; own bindings loaded at runtime instead (ADR-0014)               |
| `slint`                    | GPL or proprietary licensing                                                                                                           |
| `serde_with`               | The ADR-0003 lenient parser needs warnings, which serde adapters cannot emit                                                           |
| `chrono` / `jiff` / `time` | The only date parsed is `released`, by a small ISO 8601 reader in the core (`addon::meta`)                                             |
| `sqlx` / `redb`            | `sqlx`: async with compile-time checks, heavier than needed. `redb`: key-value, no queries. `rusqlite` chosen instead                  |
| `async-trait`              | Native async fn in traits is enough; effects are data (ADR-0001)                                                                       |
| `figment` / config crates  | No configuration file; settings live in SQLite                                                                                         |
| OpenSSL (`native-tls`)     | rustls only; banned in `.config/deny.toml`                                                                                             |

### Tools (not crate dependencies)

| Tool          | Version | Use                                 |
| ------------- | ------- | ----------------------------------- |
| cargo-nextest | 0.9.152 | CI test runner                      |
| cargo-deny    | 0.22.0  | Licenses, advisories, bans, sources |
| taplo         | 0.10.0  | TOML format and lint                |
| typos         | 1.52.2  | Spell check of code and docs        |
| cargo-machete | 0.10.1  | Unused dependencies                 |
| shellcheck    | 0.11.0  | Shell script lint                   |
| shfmt         | 3.14.1  | Shell script format                 |
| minisign      | 0.12    | Release checksum signatures; CI takes the author's static Linux build, checked by SHA-256 (`scripts/release.sh tool`), not Ubuntu's archive |

### CI actions

`actions/*` are referenced by version tag; other actions by commit, with
the version here (`docs/SECURITY.md`). `scripts/check-docs.sh` fails if a
pinned commit in `.github/` is not in this table.

| Action                        | Version | Commit                                     |
| ----------------------------- | ------- | ------------------------------------------ |
| `Swatinem/rust-cache`         | v2.9.3  | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` |
| `taiki-e/install-action`      | v2.90.8 | `fe3acb53a98b62c772db35de75ce0e4595e7ca8c` |
| `samypr100/setup-dev-drive`   | v4.1.0  | `562171fe8df7401fdf582d766cd3b6ab80d5b1a0` |
| `gradle/actions/setup-gradle` | v6.4.1  | `8da46b539cd8fe6ab27ca75128919a18d757604c` |

## Releasing

- **Versioning:** SemVer. The workspace version in the root `Cargo.toml` is
  the single source.
- **Tags:** `vMAJOR.MINOR.PATCH`, with an optional pre-release suffix
  (`v1.0.1-rc.1`). Tags containing `-` become GitHub pre-releases.

One workflow, `ci.yml`, runs everything. The builds run alongside the
checks, and `publish` waits for both:

| Trigger                  | Jobs                                                                   |
| ------------------------ | ---------------------------------------------------------------------- |
| pull request, manual run | check (linux), check (windows), check (macos), check (android)         |
| push to `main`           | the checks and build → publish `prerelease` → reproduce (android)      |
| tag `vX.Y.Z`             | the checks and build → publish `vX.Y.Z` → reproduce (android)          |

**Checks.**
- `check (linux)` runs `scripts/check.sh` (`scripts/fmt.sh --check`,
  clippy, nextest with the `ci` profile, the static checks, the release
  script tests, cargo-deny),
  then the player tests that need libmpv (`--run-ignored only`) with the
  system `libmpv2`.
- `check (windows)` runs clippy and nextest on `windows-latest`, in a copy
  of the checkout on a Dev Drive (`samypr100/setup-dev-drive`), and
  `check (macos)` runs them on `macos-latest`.
- `check (android)` runs `cargo ndk … clippy -p cineo-android` at
  `ANDROID_API`, then `./gradlew assembleDebug testDebugUnitTest lintDebug`.
- CI builds the dev profile without debug info (`CARGO_PROFILE_DEV_DEBUG=0`),
  and the linux, windows and macos checks write a `.cargo/config.toml` that
  builds the dependencies at `opt-level = 0` instead of the `Cargo.toml`'s
  2: a cold test build went from 173 s to 54 s and the tests from 6 s to
  9 s (measured locally on 2026-11-12).

**Builds.**
- `build` builds `cineo-desktop` for Linux x86_64 on `ubuntu-24.04`, never
  `ubuntu-latest`, because the runner's glibc is the oldest the binary runs
  on: `scripts/package.sh linux` fails if it needs a glibc above
  `linux_glibc_max` (2.39). v1.5.0 to v1.9.7 were built on Ubuntu 26.04
  when `ubuntu-latest` moved to it, and need glibc 2.43.
- `build` builds `cineo-desktop` for Linux x86_64 and Windows x86_64 at the
  `x86-64-v2` level (Windows with `bundled-libmpv`, after
  `scripts/fetch-libmpv.sh windows` checks the pinned archive), and one universal
  macOS binary for `aarch64-apple-darwin` and `x86_64-apple-darwin`, its
  x86_64 half also at `x86-64-v2`
  (`MACOSX_DEPLOYMENT_TARGET=11.0`, after `scripts/fetch-libmpv.sh macos`).
  The level is set per target with `CARGO_TARGET_<TRIPLE>_RUSTFLAGS`, not
  `RUSTFLAGS`, which would also reach the arm64 half.
  It puts the files into `dist/` with
  `scripts/package.sh <linux|windows|macos>`, which fails if the binary's
  `--version` is not `cineo` followed by the workspace version, and
  attests build provenance.
- On tags, `build` and `build (android)` first run
  `scripts/release.sh version <tag>`, which fails unless the tag
  without its `v` is the workspace version, so a tag pushed before the
  version bump fails before anything compiles.
- Both Android jobs set up Java, Gradle, `cargo-ndk` at
  `CARGO_NDK_VERSION` in `scripts/android.env` and the NDK named by
  `NDK_VERSION` in `scripts/libmpv-android.env` (unless the runner already
  has it) with the local action `.github/actions/setup-android`.
  `build (android)` restores libmpv from its cache and runs
  `scripts/release-android.sh`: it builds libmpv if needed, a signed
  `assembleRelease`, and runs `scripts/package.sh android`, which checks
  that the APK holds libmpv, FFmpeg, `libmpv.md` and the license texts,
  that each `.so` matches its hash, that no `.so` holds a build path or an
  OpenSSL build date (they would make the build unreproducible), that every
  `.so` has its LOAD segments aligned to at least 16 KB (`readelf`), that
  `apksigner verify` passes and the signer is the certificate the F-Droid
  recipe allows (`AllowedAPKSigningKeys`), and that the signing block has
  no dependency-info block (Gradle's is encrypted for Google;
  `dependenciesInfo` turns it off). `release-android.sh` fails if a signing
  secret is missing or the decoded keystore is empty rather than publish an
  unsigned APK.
- The `versionCode` is `MAJOR * 10000 + MINOR * 100 + PATCH` of the
  workspace version: Gradle takes it from `version-code` in `Cargo.toml`
  (`[workspace.metadata.android]`) and fails if it is not that number
  (ADR-0030).
- `build (android)` always uses the release profile's `lto = "thin"` and
  `codegen-units = 1`, so the prerelease APK is built the way F-Droid
  builds a tag. Of the other builds, only tags use it; the prerelease's
  desktop builds and `check (android)` use `lto = "off"` and
  `codegen-units = 16`.
  The `CARGO_PROFILE_RELEASE_*` variables that choose this are set on the
  build steps, never in the workflow's `env`: `Swatinem/rust-cache` hashes
  every `CARGO*` variable into its key, so a workflow-wide value that
  differs on tags made the checks on tags miss `main`'s cache.

**Caches.** Only runs on `main` save the Rust caches. `check (android)`
saves `shared-key: android` and `build (android)` saves
`shared-key: android-release`, because their release profiles differ; tag
builds start without a Rust cache. libmpv for
Android is cached by the hash of its script and `.env`, restored on tags
too and saved only by `build (android)` on `main`. After each run on
`main`, `prune caches` deletes all but the newest Rust and libmpv cache of
each job, so the repository stays under GitHub's 10 GB cache limit.

**Publishing.** `publish` runs `scripts/release.sh assets dist`,
which fails if a file of the table below is missing or `dist/` holds any
other file (`scripts/test-release.sh`, run by `scripts/check.sh`,
covers both checks), then writes `SHA256SUMS` (check a file with
`gh attestation verify <file> --repo <owner>/cineo`). It is the only job
that can write to the repository. `reproduce (android)` then builds the
pushed commit as F-Droid does and fails if it does not match the APK just
published, the release's on tags and the prerelease's on `main`
([F-Droid](#f-droid)).

The Android store listing (title, short and full description) lives in
`fastlane/metadata/android/en-US/` and six more locales (es-ES, pt-BR, ru-RU, it-IT, de-DE, fr-FR; only en-US has images), the layout F-Droid and Play read
(ADR-0030), with a 512 × 512 icon (`images/icon.png`, the brand logo on the launcher background) and six phone screenshots in `images/phoneScreenshots/` that show only Creative Commons films (their credits are in `docs/LEGAL.md`).

Every release has these files, with the same names in every release so that
`releases/latest/download/<name>` works:

| File                 | What                                                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `cineo-x86_64-linux` | The executable, built for `x86-64-v2` (`-C target-cpu=x86-64-v2`) against glibc 2.39; uses the system libmpv        |
| `cineo-x86_64-windows.exe` | The Windows executable, built for `x86-64-v2`, with `libmpv-2.dll` embedded gzip-compressed (ADR-0027)      |
| `cineo-arm64-android.apk` | The Android app for arm64-v8a, signed with the release key, with libmpv and its licenses; `libmpv.md` names the pinned sources (ADR-0029) |
| `cineo-universal-macos.dmg` | `Cineo.app` for arm64 and x86_64 (`x86-64-v2`; macOS 11 or newer), signed ad hoc, with libmpv's dylibs and their licenses (ADR-0035) |
| `mpv-dev-lgpl-x86_64-….7z` | The unmodified libmpv archive the Windows build embeds, named as upstream names it (`LIBMPV_FILE`, so the name changes with the pin); `scripts/fetch-libmpv.sh` takes it from here before upstream, which deletes old builds (ADR-0027) |
| `cineo-licenses.zip` | Cineo's license, the font licenses, `libmpv.md` and, in `libmpv/`, the license texts of every package in the Windows libmpv build (`scripts/windows/licenses/`) |
| `cineo.png`          | App icon, used by `install.sh`                                                                                      |
| `install.sh`         | Per-user installer (README, Install). Uninstall: `… \| sh -s -- --uninstall`. `CINEO_VERSION` picks another release |
| `install.ps1`        | Per-user Windows installer into `%LOCALAPPDATA%\Programs\Cineo` with a Start menu shortcut. `CINEO_UNINSTALL=1` removes it; `CINEO_VERSION` picks another release |
| `SHA256SUMS`         | Checksums of all of the above; the installer verifies them                                                          |
| `SHA256SUMS.minisig` | minisign signature of `SHA256SUMS` by the key in `minisign.pub`; `install.sh` verifies it when `minisign` is installed |

### Steps

1. On `main` with green CI, `reproduce (android)` included: bump `version`
   in `Cargo.toml`, and
   `version-code` to `MAJOR * 10000 + MINOR * 100 + PATCH`, and write the
   release notes in `docs/releases/vX.Y.Z.md` (what changed, for users;
   no download links or changelog link, those are added).
   `scripts/release.sh notes`, run by `scripts/check.sh`, fails while
   the notes of the `Cargo.toml` version are missing or empty.
2. Commit with `chore(release): vX.Y.Z`.
3. Tag `vX.Y.Z` and push the tag.
4. `ci.yml` runs every check, builds, and publishes the release
   `vX.Y.Z`. Its text comes from `scripts/release.sh body`: a line of
   download links, the notes file, and a link comparing the tag with the
   previous latest release. On tags, the build jobs check the notes before
   compiling.

### Prerelease builds

Every push to `main` builds a prerelease, alongside the checks, and
publishes it once they pass, except a push whose head commit starts with
`chore(release):`; that commit gets the real release from its tag instead.
It moves the `prerelease` tag to the pushed commit and replaces the
release's files (`gh release upload --clobber`), so the release and its
links stay in place; the first push creates it.

The desktop artifacts are not code-signed; build provenance attestations,
`SHA256SUMS` and its minisign signature are the integrity checks.
`publish` signs `SHA256SUMS` with `scripts/release.sh sign`, which fails
rather than publish unsigned checksums when the key is missing, and checks
the signature against `minisign.pub` (`scripts/test-release.sh`).
The key is the secret `CINEO_MINISIGN_KEY`, the whole secret key file of
`minisign -G -W` (no password; the secret is the protection), in the
`release-signing` environment, which deploys only from `main` and `v*` tags.
`scripts/install.sh` carries the same public key as `minisign.pub`
(`scripts/test-release.sh` runs it against signed, unsigned, re-signed and
altered releases);
`scripts/release.sh key` fails if they differ. A new key means
changing both, and installers from older releases keep the old one.

The APK is signed, because
Android requires it, with the key in these repository secrets:

| Secret | What |
| ------ | ---- |
| `ANDROID_KEYSTORE_BASE64` | The release keystore (`base64 -w0 cineo-release.jks`) |
| `ANDROID_KEYSTORE_PASSWORD` | Its password |
| `ANDROID_KEY_ALIAS` | The key's alias |
| `ANDROID_KEY_PASSWORD` | The key's password |

The owner keeps the key and a backup outside the repository. Android only
installs an update signed with the same key: if it is lost, users must
uninstall, and lose their library, to install a new one.

### F-Droid

`docs/fdroid/io.github.mdmrk.cineo.yml` is the recipe for F-Droid's
`fdroiddata` repository (ADR-0030). It passes `fdroid lint` and
`fdroid rewritemeta` (fdroidserver 2.4.5).

**Signing.** F-Droid builds the app itself from the tag and compares its
unsigned build with our signed APK from the release (`Binaries`). If they
match, it publishes our APK with our signature; if not, it publishes
nothing for that version, because `AllowedAPKSigningKeys` names only our
release certificate. F-Droid never signs Cineo with its own key, so the
F-Droid and GitHub builds update each other. For v1.9.6,
`fdroid build --on-server` in fdroidserver's `buildserver` image passed the
scanner and matched the GitHub APK (VERIFIED 2026-11-16: "compared built
binary to supplied reference binary successfully"). Why earlier releases
did not match is in ADR-0037.

**Updates.** F-Droid finds new releases by itself
(`UpdateCheckMode: Tags`): `UpdateCheckData` reads `version` and
`version-code` from `Cargo.toml` at each `vX.Y.Z` tag, and
`AutoUpdateMode: Version` adds a build for it copied from the last one,
with the tag's full commit hash. `fdroid checkupdates --auto`
(fdroidserver 2.4.5) added 1.9.7 (10907) from a test tag that bumped both
(VERIFIED 2026-11-17). The recipe installs Rust with
`scripts/install-rust.sh`, so it builds each tag with the version that
tag's `rust-toolchain.toml` pins, and cargo-ndk at that tag's
`CARGO_NDK_VERSION` (`scripts/android.env`). Only the NDK (`ndk:`) is a
literal, because F-Droid installs it before the build; `scripts/check-docs.sh`
fails if it differs from `NDK_VERSION` in `scripts/libmpv-android.env`, or
if the recipe's `UpdateCheckData` does not read the workspace version and
its `versionCode` from `Cargo.toml`. A release that changes the NDK needs
the same change in `fdroiddata` by hand.

**Reproducibility.** The build is reproducible across directories on one
machine: Rust maps `CARGO_HOME`, `RUSTUP_HOME` and the checkout to
`/cargo`, `/rustup` and `/cineo` (`--remap-path-prefix`), and libmpv is
built in the fixed `/tmp/cineo-libmpv-android` with `SOURCE_DATE_EPOCH=0`,
the NDK compilers named without their path, and OpenSSL's libraries
written into `openssl.pc` so that every `pkg-config` orders them the same
(`docs/TECHNICAL_DEBT.md`).

**Checking a build.** `scripts/check-fdroid-build.sh <commit> <release>`
builds a commit as F-Droid does and fails unless the result matches
`cineo-arm64-android.apk` of the GitHub release `<release>` (a tag, or
`prerelease`). It runs as root in fdroidserver's `buildserver` image:
`docker run --rm -v "$PWD:/cineo:ro" registry.gitlab.com/fdroid/fdroidserver:buildserver /cineo/scripts/check-fdroid-build.sh vX.Y.Z vX.Y.Z`.
It installs fdroidserver 2.4.5, writes the commit's recipe with the
commit's version and `versionCode` and that release's APK as `Binaries`
into a
`fdroiddata` directory that it commits (`fdroid build` takes
`SOURCE_DATE_EPOCH` from that commit), points `gradle:` at the image's
`gradlew-fdroid` (the one pip installs has no Gradle 9 checksums) and runs
`fdroid build --on-server` as `vagrant`. For v1.9.6 it matched, with the
recipe that installs Rust through `scripts/install-rust.sh` (VERIFIED
2026-11-17, about 15 minutes on 12 CPUs). CI runs it after `publish` on
every push, against the tag's release or the prerelease
(`reproduce (android)`); v1.9.7 matched there in 18 minutes (VERIFIED
2026-11-17). Install Rust in `~/.cargo` and
`~/.rustup`, not behind a symlink, when building by hand: `CargoNdk` maps
the canonical path of those directories, so a symlink leaves the real paths
in `libcineo_android.so` (VERIFIED 2026-11-16).

**Submission.** The first submission, `fdroiddata` merge request 57704,
was closed on 2026-11-17 only because F-Droid includes apps maintained for
more than three months; it can be sent again from 2027-01-08, three months
after v0.1.0. Before that review the owner decides whether store builds
list the Community addon catalog (ADR-0030).

## Pull requests

- Run `scripts/check.sh` before opening one.
- **AI assistance** must be disclosed in the PR description. You are
  responsible for every line you submit.

By contributing, you agree that your contributions are licensed under the
project's MIT license.
