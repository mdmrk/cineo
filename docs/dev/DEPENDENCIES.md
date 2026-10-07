# Dependencies

Every direct dependency gets a row here **when it is added**. Research
snapshot: crates.io, 2026-10-06. Versions are the latest stable at that date.
Re-check them when adding.

## In use

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
| `librqbit` (`rust-tls`, no default features) | 9.0.1 | BitTorrent session (stream, ADR-0012) | Apache-2.0 |
| `hyper` (`server`, `http1`) / `hyper-util` (`tokio`) / `http-body-util` | 1.11.1 / 0.1.21 / 0.1.5 | Loopback HTTP server for the player (stream) | MIT |
| `bytes` | 1.12.1 | Response bodies (stream) | MIT |
| `futures-util` (no default features) | 0.3.34 | Stream adapters for bodies (stream) | MIT/Apache |
| `tokio-util` (`io`) | 0.7.19 | `ReaderStream` from a torrent file reader (stream) | MIT |
| `getrandom` | 0.4.3 | Per-session path token and proxy password (stream) | MIT/Apache |

`tokio` features `process`, `io-util` and `sync` were added for the mpv
player (M3). `serde_json` is also used there for IPC messages.

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
crate-scoped exception in `deny.toml` (LEGAL.md).

UI polish (2026-10-07): not crates, but bundled assets. `cineo-desktop`
embeds Inter 4.1 (Regular and SemiBold) and DM Serif Display (Regular),
about 910 KB together, all OFL-1.1 (LEGAL.md *Bundled assets*), with
`include_bytes!`. Icons come from `iconflow` 2.1.0 (crates.io,
2026-10-07: MIT, MSRV 1.92, no runtime dependencies) with only the Tabler
pack enabled, which embeds Tabler's regular and filled icon fonts (about
1.5 MB). The other 13 packs stay off. egui's `default_fonts` stay enabled as the fallback for symbols and emoji.

## Planned (researched, not added)

| Concern | Choice | Version | License | MSRV | Added in | Why / alternatives considered |
|---------|--------|---------|---------|------|----------|-------------------------------|
| Serialization | `serde`, `serde_json` | 1.0.229 / 1.0.151 | MIT/Apache | 1.56 / 1.71 | M1 | De facto standard. `simd-json` rejected: no need |
| URL parsing | `url` | 2.5.8 | MIT/Apache | 1.63 | M1 | WHATWG-compliant; same as the reference client |
| Path encoding | `percent-encoding` | 2.3.2 | MIT/Apache | 1.51 | M1 | Needed for the `encodeURIComponent` set; already a `url` dependency |
| Versions | `semver` | 1.0.28 | MIT/Apache | 1.68 | M1 | The manifest `version` must be strict semver (reference behavior) |
| Library errors | `thiserror` | 2.0.21 | MIT/Apache | 1.77 | M1 | Typed errors without boilerplate |
| Binary errors | `anyhow` | 1.0.104 | MIT/Apache | 1.68 | M1 (CLI only) | Context chains in shells. `eyre` is equivalent; `anyhow` is more common |
| Async runtime | `tokio` | 1.53.2 | MIT | 1.71 | M1 | Required by reqwest. `smol` rejected for ecosystem fit |
| HTTP client | `reqwest` (`rustls`, `gzip`, `brotli`, `http2`; no default features) | 0.13.5 | MIT/Apache | 1.85 | M1 | Custom DNS resolver + redirect policy hooks are needed for ADR-0006. The `rustls` feature uses aws-lc-rs (its C build on Windows CI is **UNKNOWN**; fallback: `rustls-no-provider` + ring). `ureq` rejected: blocking; `hyper` directly: too low-level |
| Logging | `tracing`, `tracing-subscriber` (`env-filter`, `fmt`) | 0.1.44 / 0.3.23 | MIT | 1.65 | M1 | Spans per request; the standard choice |
| CLI | `clap` (`derive`) | 4.6.7 | MIT/Apache | 1.85 | M1 | Standard. `argh` and `lexopt` are lighter but less ergonomic |
| HTTP mocking (dev) | `wiremock` | 0.6.5 | MIT/Apache | — | M1 | Async, per-test servers, request recording. `httpmock` (1.88 MSRV) is an alternative |
| Secrets | `keyring` | 4.2.0 | MIT/Apache | 1.88 | M10 (NZB server credentials) | OS keychains. Not needed until there are credentials; cloud sync is not planned |
| Property tests (dev) | `proptest` | 1.11.0 | MIT/Apache | 1.85 | When first useful | URL round-trips, parser robustness |
| Snapshots (dev) | `insta` | 1.49.0 | Apache | 1.66 | Only if needed | Large CLI outputs |
| Fuzzing | `cargo-fuzz` / `libfuzzer-sys` | 0.13.2 / 0.4.13 | MIT/Apache (+NCSA) | — | After M2 | Parser fuzz targets |

## Deliberately not used

| Crate | Reason |
|-------|--------|
| `libmpv2` (LGPL-2.1) | External mpv process instead (ADR-0004) |
| `slint` | GPL or proprietary licensing |
| `serde_with` | The ADR-0003 lenient parser needs warnings, which serde adapters cannot emit |
| `chrono` / `jiff` / `time` | Not needed until dates are modelled (M2 `released`). Decide then; `jiff` is the current front-runner |
| `directories` 6.0.0 | Pulls in `option-ext` (MPL-2.0, not in the `deny.toml` allowlist) via `dirs-sys`. `etcetera` gives the same paths with only `cfg-if`/`windows-sys` (2026-10-06) |
| `sqlx` / `redb` | `sqlx`: async with compile-time checks, heavier than needed. `redb`: key-value, no queries. `rusqlite` chosen in M4 |
| `async-trait` | Native async fn in traits is enough; effects are data (ADR-0001) |
| `figment` / config crates | No configuration file yet |
| OpenSSL (`native-tls`) | rustls only; banned in `deny.toml` |

## Tools (not crate dependencies)

| Tool | Version | Use |
|------|---------|-----|
| cargo-nextest | 0.9.146 | CI test runner |
| cargo-deny | 0.20.2 | Licenses, advisories, bans, sources |
| zizmor | via action v0.6.4 | GitHub Actions security lint |
