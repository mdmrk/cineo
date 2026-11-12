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

1. Install the latest stable Rust with [rustup](https://rustup.rs), with
   rustfmt and clippy (ADR-0025):
   `rustup toolchain install stable --component rustfmt,clippy`.
2. Optional tools, which CI always runs:
   ```sh
   cargo install --locked cargo-nextest cargo-deny taplo-cli typos-cli cargo-machete
   ```
   `shellcheck` and `shfmt` come from your package manager. `scripts/lint.sh`
   and `scripts/fmt.sh` skip a missing tool locally and fail on it in CI.
3. libmpv (mpv 0.35 or newer) to play anything; the app loads it at
   runtime (ADR-0014). On Windows, put a `libmpv-2.dll` next to the
   executable, or build with `--features bundled-libmpv` and
   `CINEO_LIBMPV_DLL` set to the DLL's path (`scripts/fetch-libmpv.sh`
   downloads the pinned one).
4. To check the Windows build from Linux:
   `rustup target add x86_64-pc-windows-gnu`, plus the `mingw-w64-gcc`
   package for the C dependencies, then
   `cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings`.
   Run the tests with `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine`.
   The engine tests need an absolute Windows path, which a cross-compiled
   `CARGO_TARGET_TMPDIR` (`/home/…`) is not, so they fail under wine only.
   CI tests on `windows-latest` (MSVC).

5. To build the Android app (ADR-0028): the Rust target
   `rustup target add aarch64-linux-android`, `cargo install --locked
   cargo-ndk@4.1.2`, JDK 21, and the Android SDK with its licenses
   accepted (`sdkmanager --licenses`) and `ndk;30.0.16248370` installed.
   Point `ANDROID_HOME` at the SDK, or write `sdk.dir=…` to
   `android/local.properties`. Then `cd android && ./gradlew assembleDebug`
   builds `app/build/outputs/apk/debug/app-debug.apk`; `adb install -r`
   puts it on a device. Gradle's `cargoNdk` task builds `cineo-android`
   for `arm64-v8a` at API level 26, and `uniffiBindgen` writes the Kotlin
   bindings from that library into `app/build/generated/rust/kotlin`. The
   Android build keeps symbols (`CARGO_PROFILE_RELEASE_STRIP=debuginfo`)
   because `uniffi-bindgen` reads its metadata from them; AGP strips the
   copy in the APK. `./gradlew testDebugUnitTest` runs the Kotlin unit
   tests in `app/src/test/` on the JVM, and `./gradlew lintDebug` runs
   Android Lint, which fails on errors and reports warnings in
   `app/build/reports/`. CI runs `cargo ndk … clippy -p cineo-android`,
   the Gradle build, those tests and Lint (job `check (android)`).
6. To build the Android libmpv (ADR-0028):
   `ANDROID_NDK_HOME=… scripts/build-libmpv-android.sh`, with the NDK
   `scripts/libmpv-android.env` names, plus `python3` (with `venv`),
   `perl`, `make`, `ninja`, `git` and `curl` on the host. It downloads the
   pinned sources and checks their SHA-256, installs its own meson in a
   venv, cross-compiles OpenSSL, dav1d, freetype, fribidi, harfbuzz,
   libass, FFmpeg, libplacebo and mpv for `arm64-v8a` at API level 26
   in the fixed `/tmp/cineo-libmpv-android` (sources, build, prefix and
   meson; fixed so that the paths the libraries record are the same on
   every machine), and writes `target/libmpv-android/out/` (`lib/`, `licenses/`,
   `SOURCES` and `libmpv.md` with each `.so`'s SHA-256), the same as
   `libmpv-android-arm64-v8a.tar.xz`. `libmpv.md` lists the pinned
   sources with their URLs and hashes; no source archive is written
   (ADR-0029, amended 2026-11-05). It stops if FFmpeg was configured
   with `--enable-gpl`, `--enable-version3` or `--enable-nonfree`, if mpv
   has GPL code, or if a library needs one that is neither shipped nor
   part of Android. Gradle copies `out/lib/` into the APK,
   and `out/licenses/` and `libmpv.md` into `assets/licenses/`. A debug build
   without them only warns; a release build fails.
7. To build a signed release APK: set `CINEO_KEYSTORE` (the `.jks` file),
   `CINEO_KEYSTORE_PASSWORD`, `CINEO_KEY_ALIAS` and `CINEO_KEY_PASSWORD`,
   and optionally `CINEO_VERSION_CODE` (default 1), then
   `cd android && ./gradlew assembleRelease`. Without `CINEO_KEYSTORE` it
   writes an unsigned `app-release-unsigned.apk`. `ANDROID_HOME=…
   scripts/package.sh android` then copies the APK into `dist/` and
   checks it. `scripts/release-android.sh` does all of it in one command,
   the same steps CI's `build (android)` runs: it needs the four
   `CINEO_KEY*` variables and a `CINEO_VERSION_CODE` above the last
   released one, and refuses to build without them, finds the
   SDK through `ANDROID_HOME` or `android/local.properties`, rebuilds
   libmpv only when the script or `.env` changed since the last build
   (`target/libmpv-android/pins`), and leaves
   `dist/cineo-arm64-android.apk`. Release builds run R8 with the keep
   rules in `android/app/proguard-rules.pro` for JNA, the UniFFI bindings
   and rustls-platform-verifier, and store the `.so` files compressed.
   AGP keeps the libmpv and FFmpeg `.so` files unstripped, so they match
   the hashes in `libmpv.md`.

If you build with a distro-packaged Rust instead of rustup, make sure it is a
current stable release. Some distro builds expect a cross linker name; if
linking fails with `linker 'x86_64-linux-gnu-gcc' not found`, set
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc` in your shell. Do not commit
that setting.

## Everyday commands

| Task                 | Command                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------- |
| Everything CI checks | `scripts/check.sh` (`--quick` skips lint, docs and deny)                                        |
| Run the desktop app  | `cargo run -p cineo-desktop -- -v` (`--data-dir`, `--cache-dir`, `--allow-private-network`) |
| Format everything    | `scripts/fmt.sh` (Rust, TOML, shell; `--check` only verifies)                               |
| Static checks        | `scripts/lint.sh` (taplo, shellcheck, typos, cargo-machete, doc references, lockfile)                       |
| Lint                 | `cargo clippy --workspace --all-targets --locked -- -D warnings`                            |
| Test                 | `cargo nextest run --workspace` or `cargo test --workspace`                                 |
| One test             | `cargo test -p cineo-core some_name`                                                        |
| Docs                 | `cargo doc --workspace --no-deps --open`                                                    |
| Dependency policy    | `cargo deny --config .config/deny.toml check`                                               |

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
- `unsafe_code = "forbid"`, except in the modules listed in [SECURITY.md](SECURITY.md), which also
  describes the exception process.

### New stable Rust releases (ADR-0025)

CI always uses the latest stable Rust. When a release brings new lints, fix
them.

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
- GUI: `cineo-desktop` logs to stderr. `-v` gives debug logs for Cineo
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

1. Install the addon in `cineo-desktop -v`. Every parse **warning** is
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
  `$XDG_DATA_HOME/cineo/cineo.db` (`~/.local/share/cineo/cineo.db`);
  `--data-dir <dir>` uses another directory. The database uses write-ahead
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
3. The version (`cineo-desktop --version`) and OS.
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
| 1   | Start `cineo-desktop --data-dir <tmp> -v`                                          | Board shows Cinemeta rows with posters                                                       |
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
| `serde_json`                                                                           | 1.0.154                                                                                                                                | JSON documents (core)                                                                                                                           | MIT/Apache                           |
| `thiserror`                                                                            | 2.0.21                                                                                                                                 | Library errors                                                                                                                                  | MIT/Apache                           |
| `url`                                                                                  | 2.5.8                                                                                                                                  | URLs (core, net)                                                                                                                                | MIT/Apache                           |
| `reqwest`                                                                              | 0.13.5                                                                                                                                 | Addon HTTP client (net)                                                                                                                         | MIT/Apache                           |
| `tokio`                                                                                | 1.54.1                                                                                                                                 | Async runtime (net, shells)                                                                                                                     | MIT                                  |
| `tracing` / `tracing-subscriber`                                                       | 0.1.44 / 0.3.23                                                                                                                        | Logging                                                                                                                                         | MIT                                  |
| `anyhow`                                                                               | 1.0.104                                                                                                                                | Shell errors (cli)                                                                                                                              | MIT/Apache                           |
| `clap`                                                                                 | 4.6.7                                                                                                                                  | CLI arguments (cli)                                                                                                                             | MIT/Apache                           |
| `wiremock` (dev)                                                                       | 0.6.5                                                                                                                                  | Mock addon servers in tests                                                                                                                     | MIT/Apache                           |
| `tempfile` (dev)                                                                       | 3.27.0                                                                                                                                 | Temporary directories removed after each test                                                                                                   | MIT/Apache                           |
| `rusqlite` (`bundled`, no default features)                                            | 0.41.1                                                                                                                                 | SQLite database (store)                                                                                                                         | MIT; bundled SQLite is public domain |
| `etcetera`                                                                             | 0.11.0                                                                                                                                 | XDG data and cache directories (store)                                                                                                          | MIT/Apache                           |
| `eframe` (`glow`, `default_fonts`, `x11`, `wayland`, `accesskit`, `links`; no default features) | 0.36.2 (ADR-0033)                                                                                                                                 | Desktop window and egui (desktop, ADR-0011)                                                                                                     | MIT/Apache                           |
| `iconflow` (`pack-tabler` only; no default features)                                   | 2.1.0                                                                                                                                  | Tabler icon font and codepoints for the desktop UI (desktop)                                                                                    | MIT; Tabler font MIT                 |
| `image` (`jpeg`, `png`, `webp` only)                                                   | 0.25.10                                                                                                                                | Image header checks and decoding (desktop)                                                                                                      | MIT/Apache                           |
| `fast_image_resize` (default features)                                                 | 6.1.0                                                                                                                                  | SIMD downscaling of posters and backdrops (desktop)                                                                                             | MIT/Apache                           |
| `egui_kittest` (dev, no default features)                                              | 0.36.2 (ADR-0033)                                                                                                                                 | Headless UI tests via AccessKit (desktop)                                                                                                       | MIT/Apache                           |
| `librqbit` (`rust-tls`, `disable-upload`, no default features)                         | 9.1.0 + our patch, from branch `main` of the [fork](https://github.com/mdmrk/rqbit) (upstream `main` merged with `feat/stream-window`) | BitTorrent session (stream, ADR-0012)                                                                                                           | Apache-2.0                           |
| `zbus` (`async-io`; no default features; not on Windows)                  | 5.19.0, the version `accesskit_unix` and fastframe use, so only one builds                                              | Keeps the screen awake during playback over the session D-Bus (desktop)                                                                         | MIT                                  |
| `fastframe-now-playing` (git, tag; not on crates.io)                                   | 0.17.0                                                                                                                                 | Media keys and the desktop's media controls: MPRIS through `mpris-server` 0.10.0 on Linux, SMTC through `souvlaki` 0.8.3 on Windows (desktop, ADR-0031) | MIT (`mpris-server`: MPL-2.0)        |
| `fastframe-instance` (git, tag; not on crates.io) | 0.17.0 | Hands a link from a new launch to the running window (desktop, ADR-0032) | MIT |
| `fastframe-file` (git, tag; not on crates.io) | 0.17.0 | Replaces the bundled libmpv, the link registration entry and the torrent blocklist without leaving a half-written file (desktop, stream, ADR-0032) | MIT |
| `fastframe-power` (git, tag; not on crates.io) | 0.17.0 | Blocks idle system sleep during playback and pauses before the computer sleeps (desktop, ADR-0032); the screen stays on through `zbus` and `keepawake` | MIT |
| `fastframe-log` (git, tag; no default features) | 0.17.0 | Records panics in `panic.log` without their message (desktop, ADR-0032) | MIT |
| `fastframe-emoji` (git, tag; not on crates.io) | 0.17.0 | Colour emoji in egui text (desktop, ADR-0033) | MIT |
| `fastframe-fonts` (git, tag; no default features) | 0.17.0 | Installed fonts as fallbacks for other scripts (desktop, ADR-0033) | MIT |
| `fastframe-text` (`watch`; git, tag; not on crates.io) | 0.17.0 | Follows the desktop's font rendering and text size, live (desktop, ADR-0033) | MIT |
| `fastframe-scroll` (git, tag; not on crates.io) | 0.17.0 | Touchpad scrolling with glide on Linux (desktop, ADR-0033) | MIT |
| `fastframe-graphics` (git, tag; not on crates.io) | 0.17.0 | Explains in a dialog when no OpenGL window can open (desktop, ADR-0033) | MIT |
| `keepawake` (Windows only)                                                             | 0.6.1                                                                                                                                  | Keeps the screen awake during playback on Windows with `SetThreadExecutionState` (desktop, ADR-0027)                                           | MIT                                  |
| `hyper` (`server`, `http1`) / `hyper-util` (`tokio`) / `http-body-util`                | 1.12.0 / 0.1.21 / 0.1.5                                                                                                                | Loopback HTTP server for the player (stream)                                                                                                    | MIT                                  |
| `bytes`                                                                                | 1.12.1                                                                                                                                 | Response bodies (stream)                                                                                                                        | MIT                                  |
| `futures-util` (no default features)                                                   | 0.3.34                                                                                                                                 | Stream adapters for bodies (stream)                                                                                                             | MIT/Apache                           |
| `tokio-util` (`io`)                                                                    | 0.7.20                                                                                                                                 | `ReaderStream` from a torrent file reader (stream)                                                                                              | MIT                                  |
| `getrandom`                                                                            | 0.4.3                                                                                                                                  | Per-session path token and proxy password (stream)                                                                                              | MIT/Apache                           |
| `rust-i18n`                                                                            | 4.2.4                                                                                                                                  | UI message catalogs, embedded at compile time (desktop, ADR-0021)                                                                               | MIT                                  |
| `sys-locale`                                                                           | 0.3.2                                                                                                                                  | The OS locale for the default UI language (desktop)                                                                                             | MIT/Apache                           |
| `raw-window-handle`                                                                    | 0.6.2                                                                                                                                  | The window's Wayland or Xlib display for mpv's hardware-decoding interop, and the HWND for SMTC on Windows (desktop); already in the tree through eframe | MIT/Apache/Zlib                      |
| `libloading`                                                                           | 0.9.0                                                                                                                                  | Loads libmpv at runtime for embedded playback (player, ADR-0014). glutin still pulls in 0.8.9 on Linux, so the tree has both (as of 2026-10-07) | ISC                                  |
| `uniffi` (`cli` only with the `bindgen` feature)                                       | 0.32.2                                                                                                                                 | Kotlin bindings for the Android shell (android, ADR-0028)                                                                                        | MPL-2.0 (LEGAL.md)                   |
| `jni` (no default features; Android only)                                              | 0.22.4                                                                                                                                 | The one JNI entry point that hands the app `Context` to the TLS verifier (android)                                                              | MIT/Apache                           |
| `rustls-platform-verifier` (Android only)                                              | 0.7.1, already in the tree through reqwest                                                                                             | `android::init_with_env` for Android's certificate verifier (android)                                                                           | MIT/Apache                           |
| `android_logger` (no default features; Android only) | 0.15.1 | Sends logs to logcat (android) | MIT/Apache |
| `log` (Android only) | 0.4.35 | Maximum level for `android_logger`; `tracing` forwards to it through its `log` feature (android) | MIT/Apache |

`tokio` features `io-util` and `sync` serve the mpv player; `process` was
dropped with the external mpv player (2026-10-07). The player uses
`serde_json` for mpv's track list.

`rusqlite` drops its default features (`cache`, and an FFI backend used only
on wasm). `bundled` compiles SQLite from source, so no system library is
needed. `etcetera` MSRV: 1.87.

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
the blocklist before metadata is asked for (ADR-0026). To update: merge
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
| `directories` 6.0.0        | Pulls in `option-ext` (MPL-2.0, not in the `.config/deny.toml` allowlist) via `dirs-sys`. `etcetera` gives the same paths (2026-10-06) |
| `sqlx` / `redb`            | `sqlx`: async with compile-time checks, heavier than needed. `redb`: key-value, no queries. `rusqlite` chosen instead                  |
| `async-trait`              | Native async fn in traits is enough; effects are data (ADR-0001)                                                                       |
| `figment` / config crates  | No configuration file; settings live in SQLite                                                                                         |
| OpenSSL (`native-tls`)     | rustls only; banned in `.config/deny.toml`                                                                                             |

### Tools (not crate dependencies)

| Tool          | Version | Use                                 |
| ------------- | ------- | ----------------------------------- |
| cargo-nextest | 0.9.150 | CI test runner                      |
| cargo-deny    | 0.20.2  | Licenses, advisories, bans, sources |
| taplo         | 0.10.0  | TOML format and lint                |
| typos         | 1.51.1  | Spell check of code and docs        |
| cargo-machete | 0.9.2   | Unused dependencies                 |
| shellcheck    | 0.11.0  | Shell script lint                   |
| shfmt         | 3.14.1  | Shell script format                 |

## Releasing

- **Versioning:** SemVer. The workspace version in the root `Cargo.toml` is
  the single source.
- **Tags:** `vMAJOR.MINOR.PATCH`, with an optional pre-release suffix
  (`v1.0.1-rc.1`). Tags containing `-` become GitHub pre-releases.

One workflow, `ci.yml`, runs everything; each job waits for the one before
it:

| Trigger                  | Jobs                                 |
| ------------------------ | ------------------------------------ |
| pull request, manual run | check (linux), check (windows), check (android) |
| push to `main`           | the checks → build → publish `prerelease` |
| tag `vX.Y.Z`             | the checks → build → publish `vX.Y.Z`     |

The checks show on GitHub as `ci / check (linux)`, `ci / check (windows)`,
`ci / check (android)`, `ci / build (linux)`, `ci / build (windows)`,
`ci / build (android)` and `ci / publish`.
`check (linux)` runs `scripts/check.sh` (`scripts/fmt.sh --check`, clippy,
nextest with the `ci` profile, `scripts/lint.sh`, docs, cargo-deny); `check (windows)` runs clippy and nextest on
`windows-latest`. `build` builds `cineo-desktop` for Linux x86_64 and
Windows x86_64 at the `x86-64-v2` level (Windows with `bundled-libmpv`,
after `scripts/fetch-libmpv.sh` checks the pinned archive), puts the files
into `dist/` with `scripts/package.sh <linux|windows>` and attests build
provenance. Both Android jobs set up Java, Gradle, `cargo-ndk` and the NDK
named by `NDK_VERSION` in `scripts/libmpv-android.env` with the local action
`.github/actions/setup-android` and share one Rust cache (`shared-key: android`), which
`check (android)` fills first and tag builds do not use. `build (android)` restores libmpv from the cache (keyed by
the hash of the script and its `.env`, never on tags) and runs
`scripts/release-android.sh`: it builds libmpv if needed, a signed
`assembleRelease`, and runs `scripts/package.sh android`, which checks that
the APK holds libmpv, FFmpeg, `libmpv.md` and the license texts, that each
`.so` matches its hash, that no `.so` holds a build path or an OpenSSL build
date (they would make the build unreproducible), that every `.so` in the APK has its LOAD segments
aligned to at least 16 KB (`readelf`), that `apksigner verify` passes, that
the signing block has no dependency-info block (Gradle's is encrypted for
Google, and IzzyOnDroid flags it; `dependenciesInfo` turns it off), and
that the
`versionCode` is `MAJOR * 10000 + MINOR * 100 + PATCH` of the workspace
version, which Gradle computes (ADR-0030). The script fails if a signing
secret is missing or the decoded keystore is empty rather than publish an
unsigned APK; `check (android)` builds an
unsigned debug APK on pull requests. `publish` writes `SHA256SUMS` (check a file with
`gh attestation verify <file> --repo <owner>/cineo`). Only `publish` can
write to the repository. The checks and prerelease builds restore the compiled
dependencies from the cache saved by the last run on `main`; tag builds
start without a cache.

The Android store listing (title, short and full description) lives in
`fastlane/metadata/android/en-US/`, the layout F-Droid and IzzyOnDroid read
(ADR-0030), with a 512 × 512 icon (`images/icon.png`, the brand logo on the launcher background) and six phone screenshots in `images/phoneScreenshots/` that show only Creative Commons films (their credits are in `docs/LEGAL.md`).

`docs/fdroid/io.github.mdmrk.cineo.yml` is the draft build recipe for
F-Droid's `fdroiddata` repository (ADR-0030). It passes `fdroid lint`
(fdroidserver 2.4.5, against `fdroiddata`'s `config/` on 2026-11-05) but
has never been built by F-Droid, which publishes only an APK that
reproduces ours (`Binaries`, `AllowedAPKSigningKeys`). The build is
reproducible across directories on one machine: Rust maps `CARGO_HOME`,
`RUSTUP_HOME` and the checkout to `/cargo`, `/rustup` and `/cineo`
(`--remap-path-prefix`), and libmpv is built in the fixed
`/tmp/cineo-libmpv-android` with `SOURCE_DATE_EPOCH=0` and the NDK
compilers named without their path (`docs/TECHNICAL_DEBT.md`). Updates are manual
(`UpdateCheckMode: None`), because F-Droid reads the version with a regular
expression and Gradle computes it. Each release changes `versionName`,
`versionCode`, `commit`, `CurrentVersion` and `CurrentVersionCode`, and the
Rust and cargo-ndk versions the release was built with, which
`scripts/release-android.sh` prints as its `F-Droid recipe:` line.

Every release has these files, with the same names in every release so that
`releases/latest/download/<name>` works:

| File                 | What                                                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `cineo-x86_64-linux` | The executable, built for `x86-64-v2` (`-C target-cpu=x86-64-v2`); uses the system libmpv                           |
| `cineo-x86_64-windows.exe` | The Windows executable, built for `x86-64-v2`, with `libmpv-2.dll` embedded (ADR-0027)                      |
| `cineo-arm64-android.apk` | The Android app for arm64-v8a, signed with the release key, with libmpv and its licenses; `libmpv.md` names the pinned sources (ADR-0029) |
| `cineo-licenses.zip` | Cineo's license, the font licenses, LGPL-2.1 and `libmpv.md`                                                        |
| `cineo.png`          | App icon, used by `install.sh`                                                                                      |
| `install.sh`         | Per-user installer (README, Install). Uninstall: `… \| sh -s -- --uninstall`. `CINEO_VERSION` picks another release |
| `install.ps1`        | Per-user Windows installer into `%LOCALAPPDATA%\Programs\Cineo` with a Start menu shortcut. `CINEO_UNINSTALL=1` removes it; `CINEO_VERSION` picks another release |
| `SHA256SUMS`         | Checksums of all of the above; the installer verifies them                                                          |

### Steps

1. On `main` with green CI: bump `version` in `Cargo.toml`.
2. Commit with `chore(release): vX.Y.Z`.
3. Tag `vX.Y.Z` and push the tag.
4. `ci.yml` runs every check, builds, and publishes the release
   `vX.Y.Z` with notes generated from the commits since the previous tag.

### Prerelease builds

Every push to `main` builds a prerelease once the checks pass, except a push
whose head commit starts with `chore(release):`; that commit gets the real
release from its tag instead. It deletes the previous `prerelease` GitHub
release and its tag and recreates both at the pushed commit.

The desktop artifacts are not code-signed; build provenance attestations
and `SHA256SUMS` are the integrity checks. The APK is signed, because
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

## Pull requests

- Run `scripts/check.sh` before opening one.
- **AI assistance** must be disclosed in the PR description. You are
  responsible for every line you submit.

By contributing, you agree that your contributions are licensed under the
project's MIT license.
