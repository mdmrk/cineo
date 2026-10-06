---
paths:
  - "crates/cineo-net/**"
  - "crates/cineo-player-mpv/**"
  - "crates/cineo-store/**"
  - "crates/cineo-desktop/**"
  - "crates/cineo-core/src/addon/**"
  - "docs/SECURITY.md"
  - ".github/workflows/**"
---

# Security rules (docs/SECURITY.md is authoritative)

- Treat all addon-originated data as hostile: URLs, JSON, headers, sizes,
  redirects, timing.
- HTTP:
  - Only through `cineo-net`'s `AddonClient`/`NetPolicy`.
  - Never build a second `reqwest::Client`.
  - Never enable system proxies.
  - Never relax the defaults (private networks off, size cap, timeouts,
    redirect checks) except through an explicit user-facing option.
- Never log or put into error messages full addon/transport URLs, stream
  URLs with tokens, or `proxyHeaders` values. Log the origin + path instead.
- Player:
  - Typed commands only.
  - Media is loaded via IPC `loadfile` with JSON arguments.
  - Only `http(s)` URLs from addons.
  - Never pass addon strings as mpv options or command-line arguments.
  - Header names must be tokens; values must have no CR/LF/NUL.
- Filesystem: no paths derived from addon data. SQL uses bound parameters
  only. Secrets go to the OS keyring.
- No shell invocation with external input.
- Workflows:
  - Pin actions by full commit SHA (with a version comment).
  - Default `permissions: contents: read`.
  - `persist-credentials: false`.
  - Pass inputs through `env:`, never by interpolating into `run:`.
- Any change to a security default updates `docs/SECURITY.md` and needs a
  test that pins the new behavior.
