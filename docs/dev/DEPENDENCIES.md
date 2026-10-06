# Dependencies

Every direct dependency gets a row here **when it is added**. Research
snapshot: crates.io, 2026-10-06. Versions are the latest stable at that date.
Re-check them when adding.

## In use

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| — | — | No dependencies yet (M0) | — |

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
| Database | `rusqlite` (`bundled`) | 0.40.2 | MIT | — | M4 | Library queries (continue watching) want SQL. `sqlx` (async, compile-time checks, heavier) and `redb` (KV, no queries) considered |
| Platform dirs | `directories` | 6.0.0 | MIT/Apache | — | M4 | Config, data and log paths per OS |
| Secrets | `keyring` | 4.2.0 | MIT/Apache | 1.88 | Sync (future) | OS keychains. Not needed until there are credentials |
| Desktop UI | `tauri` | 2.12.1 | Apache/MIT | 1.95 | M5 (ADR-0005, proposed) | See the ADR for egui, iced and Slint |
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
| `async-trait` | Native async fn in traits is enough; effects are data (ADR-0001) |
| `figment` / config crates | No configuration file yet |
| OpenSSL (`native-tls`) | rustls only; banned in `deny.toml` |

## Tools (not crate dependencies)

| Tool | Version | Use |
|------|---------|-----|
| cargo-nextest | 0.9.146 | CI test runner |
| cargo-deny | 0.20.2 | Licenses, advisories, bans, sources |
| zizmor | via action v0.6.4 | GitHub Actions security lint |
