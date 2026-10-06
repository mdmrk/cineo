# Addon protocol fixtures

Hand-written JSON documents for protocol tests. None exist yet. They arrive
with Milestone 1 (see `docs/ROADMAP.md` and `docs/TESTING.md`).

**Provenance rule:** fixtures are written by Cineo contributors from the
public protocol documentation (<https://stremio.github.io/stremio-addon-sdk/>)
and from behavior observed in the MIT-licensed reference client
`stremio-core`. Do not copy real addon responses. If a fixture is ever derived
from one, record the source URL, the date and the license/permission here,
and strip anything user-specific.

Planned layout:

| Directory | Purpose |
|-----------|---------|
| `basic/`  | A well-formed addon using the common protocol surface. |
| `quirks/` | Malformed-but-tolerated data seen in the wild: must parse with warnings. |
| `invalid/`| Documents that must be rejected with a specific error. |

Every fixture must be referenced by at least one test.
