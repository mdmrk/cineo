# Compatibility

This file tracks what actually works. Nothing is claimed without evidence.

| Status | Meaning |
|--------|---------|
| **Verified** | Automated test **and** checked against real addons or players (date in Notes) |
| **Supported** | Implemented, with automated tests |
| **Partial** | Implemented with known gaps (listed in Notes) |
| **Experimental** | Implemented, untested or behind a flag |
| **Planned** | On the roadmap (milestone in Notes) |
| **Unsupported** | Deliberately not supported (reason in Notes) |
| **Unknown** | Not yet investigated |

Rules:
- A status can only move up when tests exist. Name the test in Notes.
- When behavior changes, update this file in the same commit.
- A downgrade, for example when a real addon reveals a gap, is welcome. Do
  it immediately.

_Last reviewed: 2026-10-06. No product code exists yet; everything is Planned
or a decision._

## Addon protocol

| Feature | Status | Notes |
|---------|--------|-------|
| HTTP transport (`…/manifest.json`) | Planned | M1 |
| Legacy transport (`/stremio/v1`) | Unsupported | ADR-0007 |
| IPFS/IPNS transport | Unsupported | ADR-0007 |
| Manifest parsing and validation | Planned | M1 |
| Catalog `extra` (full and short form) | Planned | M1 |
| Resource filtering (`types`, `idPrefixes`) | Planned | M1; reference semantics in ADDON_PROTOCOL.md |
| Catalog requests: `genre`, `skip`, `search` | Planned | M1 |
| Catalog response (`metas`) | Planned | M1 |
| Meta response, videos/episodes | Planned | M2 |
| Stream response: `url` (http/https) | Planned | M2 |
| Stream response: `ytId` | Unknown | Needs a YouTube resolution strategy |
| Stream response: `externalUrl` | Planned | M2; opened in the system browser after confirmation |
| Stream response: `infoHash` (torrent) | Unsupported | Needs a streaming engine; future research (GOALS.md) |
| Stream response: archives, `nzbUrl` | Unsupported | Needs a streaming engine |
| Stream `behaviorHints.proxyHeaders` | Planned | M3; validated headers only |
| Stream `behaviorHints.bingeGroup` | Planned | v0.x binge-watching |
| Subtitles resource | Planned | M6 |
| Subtitles in stream objects | Planned | M6 |
| `addon_catalog` resource | Planned | v0.x |
| Addon configuration (`config`, `configurable`) | Planned | v0.x; opens the addon's `/configure` page |
| `behaviorHints.adult` / `p2p` warnings | Planned | M1 parses, M5 shows |
| Response caching (`Cache-Control`) | Planned | v0.x |
| Meta `links`, `trailers` | Planned | v0.x |
| Native EPG (`epgProvider`, scheduled videos) | Unsupported | Future research |

## Application features

| Feature | Status | Notes |
|---------|--------|-------|
| Install/remove/order addons | Planned | M4 (persisted) |
| Board (browsable catalogs) | Planned | M5 |
| Discover with filters | Planned | M5 |
| Search across addons | Planned | M2 (aggregation), M5 (UI) |
| Detail page | Planned | M5 |
| Stream list aggregated across addons | Planned | M2 |
| Playback via external mpv | Planned | M3 |
| Embedded playback in the window | Planned | Future (libmpv render API) |
| Audio and subtitle track selection | Planned | M3 |
| Library | Planned | M4 |
| Watch progress / continue watching | Planned | M4 |
| Deep links (`cineo://`) | Planned | v0.x |
| `stremio://` addon install links | Unknown | Legal and UX question (GOALS.md) |
| Casting | Planned | Future research |
| Account sync | Planned | Future research; own protocol |

## Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Linux x86_64 | Planned | Primary target |
| Windows x86_64 | Planned | CI from M0 |
| macOS aarch64 | Planned | CI from M0; best-effort until v1.0 |
| Android / iOS / TV / Web | Unsupported | Postponed (GOALS.md) |

## Verified addons

Real addons Cineo has been checked against. Add a row whenever you verify one.
Only list public addons, never configured URLs.

| Addon | Manifest URL | Checked | Result |
|-------|-------------|---------|--------|
| — | — | — | — |
