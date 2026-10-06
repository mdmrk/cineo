# 0010. Stremio behavioral parity; own local streaming engine

- Status: Accepted (goal and approach); engine library choice **Proposed** until the M9 spike
- Date: 2026-10-06

## Context
Owner decisions, 2026-10-06:
- The product goal is the **same behavior as Stremio**.
- `stremio://` deep links are handled.
- No cloud sync for now.

In Stremio, non-URL stream sources go through a separately distributed
**streaming server**, a local HTTP service on `127.0.0.1:11470`. This covers
torrents (`infoHash`), archives (`rarUrls`, `zipUrls`, `7zipUrls`, `tgzUrls`,
`tarUrls`) and `nzbUrl`. (VERIFIED-DOC: SDK `stream.md` and `subtitles.md`
tips. VERIFIED: its code is not in the open repositories; stremio-shell-ng
downloads a `server.js`.) ADR-0007 rejected depending on that server.
ADR-0007 left torrent support as an open research question; this ADR
settles it.

mpv plays HTTP(S) URLs with range requests well. It cannot play magnet links
or archive members by itself (INFERRED from mpv's protocol list).

Candidate libraries (crates.io, 2026-10-06):
- `librqbit` 9.0.1: Apache-2.0, updated 2026-08, BitTorrent client library
  with streaming support. Its suitability is to be verified in a spike.
- `cratetorrent` 0.1.0: unmaintained since 2020.

## Options considered
1. Require or bundle Stremio's streaming server. Its license is UNKNOWN and
   the code is not open; this contradicts ADR-0007.
2. Hand sources to an external torrent client. Poor UX, no parity.
3. **Our own local streaming engine.** A new IO crate, `cineo-stream`, that
   turns a non-URL `StreamSource` into a loopback HTTP URL with range support
   for the player.
4. Skip these sources. This contradicts the parity goal.

## Decision
- **Goal.** Stremio-equivalent user-facing behavior on the desktop. The
  exclusions are cloud/account sync (not planned for now) and anything that
  needs Stremio's private services or assets. This replaces the narrower
  goal wording in GOALS.md.
- **Engine.** Option 3. `cineo-stream` sits behind the "stream resolver"
  extension point (ARCHITECTURE.md). Its contract:
  `resolve(StreamSource) -> PlayableUrl` plus progress and status events.
  The core models sources and decisions; the engine performs IO.
- **Phasing** (ROADMAP.md): HTTP URLs (M3) → torrents (M9) → archives,
  then NZB (M10). `ytId` gets its own small decision in v0.x; the strategy is
  UNKNOWN, with mpv's ytdl hook restricted to validated YouTube ids as one
  candidate (SECURITY.md).
- **Library.** `librqbit` is the leading candidate for torrents, confirmed
  or replaced by a spike at the start of M9. Archive and NZB support are
  researched at M10 start.
- **Safety** (SECURITY.md §Streaming engine):
  - The engine binds to loopback only, with an unguessable per-session path
    token.
  - Cache size is bounded.
  - P2P is disclosed before first use and can be disabled entirely
    (LEGAL.md).

## Consequences
- This is the largest single subsystem, and the MVP does not wait for it.
  The MVP remains HTTP-URL playback (M1–M7). Parity work follows as
  milestones.
- A local HTTP server becomes part of the attack surface and needs its own
  tests: binding, token, path handling.
- Disk usage and network behavior need user-visible settings and
  diagnostics (DEBUGGING.md).

## Rejected alternatives
- Option 1: license unknown, code closed, and against ADR-0007.
- Option 2: does not deliver the behavior.
- Option 4: contradicts the owner's goal.
