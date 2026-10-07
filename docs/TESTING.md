# Testing

**Principle:** a behavior is not supported until a test proves it. Bug fixes
start with a failing regression test. Add test infrastructure only when the
first test needs it.

## Pyramid

```
            ┌───────────────┐  few: E2E (CLI binary vs mock addon; later GUI smoke)
          ┌─┴───────────────┴─┐  some: integration (IO crate vs local mock / temp DB / fake mpv)
      ┌───┴───────────────────┴───┐  many: unit + protocol fixtures (pure core, fast)
```

| Layer | Where | Tools | Runs in CI |
|-------|-------|-------|-----------|
| Unit | `#[cfg(test)] mod tests` next to the code | std test harness | always |
| Protocol fixtures | `crates/cineo-core/tests/` + `tests/fixtures/addons/` | JSON fixtures | always |
| Network integration | `crates/cineo-net/tests/` | local mock HTTP server (`wiremock`, chosen in M1) | always |
| Persistence | `crates/cineo-store/tests/` | temp-dir SQLite, fixture DBs per schema version | always (M4+) |
| Player | `crates/cineo-player-mpv/tests/`, `src/embedded/tests.rs` | fake IPC server for unit tests; real mpv and libmpv tests marked `#[ignore]` (`cargo test -p cineo-player-mpv -- --ignored`) | fake: always; real: nightly/manual |
| E2E | `crates/cineo-cli/tests/` | built binary (`CARGO_BIN_EXE_cineo`) + mock addon | always |
| Live addons | `scripts/` (manual) | real public addons | never in CI (flaky, external) |

## Rules

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

## Not set up yet

Added only with a concrete first use: property tests (`proptest`, e.g. URL
encoding round-trips), fuzzing (`cargo-fuzz`; targets: the manifest, catalog,
meta and stream parsers and mpv IPC decoding; nightly, not per PR), snapshot
tests (`insta`, only for large structured output), benchmarks (only for a
measured problem), and a replayed compatibility corpus (v1.0 goal).

Commands are in [CONTRIBUTING.md](../CONTRIBUTING.md#everyday-commands).

## Per-subsystem strategy

| Subsystem | Strategy |
|-----------|----------|
| Protocol parsing | Fixtures for basic, quirks and invalid cases; every lenient rule has a fixture case; fuzz later |
| URL building | Table tests including the docs' example, reserved chars, unicode, prefix and query transport URLs |
| Filtering/planning | Fixture manifests × request matrix |
| Network policy | Pure IP/URL classification tables + mock-server tests (blocked loopback, redirects, size, gzip bomb, timeout, no URL leak in errors) |
| Aggregation (M2) | Reducer tests: effects emitted, partial failure, stale responses dropped |
| Player (M3) | Fake IPC peer asserting exact JSON sent; event decoding tables; real-mpv smoke `#[ignore]` |
| Persistence (M4) | Round-trip; migration from each historical schema fixture; corruption handling |
| GUI (M5) | Core-level tests carry the logic; UI smoke test only |
