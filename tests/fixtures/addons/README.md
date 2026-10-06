# Addon protocol fixtures

Hand-written JSON documents used by protocol tests. **Provenance: written by
Cineo contributors from the public protocol documentation**
(<https://stremio.github.io/stremio-addon-sdk/>) and from behavior observed
in the MIT-licensed reference client `stremio-core`. No fixture is copied from
a real addon's responses; titles and ids are illustrative.

| Directory | Purpose |
|-----------|---------|
| `basic/`  | A well-formed addon using the common protocol surface. |
| `quirks/` | Malformed-but-tolerated data seen in the wild: must parse with warnings. |
| `invalid/`| Documents that must be rejected with a specific error. |

Rules:
- One behavior per fixture where practical; name files after what they test.
- Every fixture is referenced by at least one test (see
  `crates/cineo-core/tests/protocol_fixtures.rs`).
- If a fixture is derived from a real addon response, record the source URL,
  date and license/permission here, and strip anything user-specific.
