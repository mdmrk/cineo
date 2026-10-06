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

_Last reviewed: 2026-10-06._

## Addon protocol

| Feature | Status | Notes |
|---------|--------|-------|
| HTTP transport (`…/manifest.json`) | Verified | `request::tests::*`, `addon_client::fetches_manifest_and_catalog_page`; live: Cinemeta 2026-10-06 |
| Legacy transport (`/stremio/v1`) | Unsupported | ADR-0007; excluded from the parity goal (owner decision 2026-10-06) |
| IPFS/IPNS transport | Unsupported | ADR-0007 |
| Manifest parsing and validation | Verified | `protocol_fixtures::{basic_manifest_*, quirky_manifest_*, invalid_manifests_*}`; live: Cinemeta, OpenSubtitles v3 |
| Catalog `extra` (full and short form) | Supported | `quirky_manifest_parses_with_expected_warnings`, `options_limit_bounds_repeated_extra_values` |
| Resource filtering (`types`, `idPrefixes`) | Supported | `short_resource_inherits_*`, `full_resource_uses_only_its_own_filters`, `catalog_support_checks_*` |
| Catalog requests: `genre`, `skip`, `search` | Verified | `encodes_extra_like_encode_uri_component`; live: Cinemeta `genre`, `search` |
| Catalog response (`metas`) | Verified | `basic_catalog_parses`, `quirky_catalog_*`, `null_metas_*`; live: Cinemeta |
| Meta response, videos/episodes | Supported | `series_meta_parses_and_sorts_videos`, `movie_meta_without_videos_*`, `fetches_meta_and_streams` |
| Stream response: `url` (http/https) | Supported | `stream_sources_are_recognized_in_reference_order`; playback in M3 |
| Stream response: `ytId` | Planned | v0.x; resolution strategy UNKNOWN (ADR-0010) |
| Stream response: `externalUrl` | Planned | M2; opened in the system browser after confirmation |
| Stream response: `infoHash` (torrent), `fileIdx`, `sources` | Planned | M9, own streaming engine (ADR-0010) |
| Stream response: archives (`rarUrls`, `zipUrls`, `7zipUrls`, `tgzUrls`, `tarUrls`) | Planned | M10 (ADR-0010) |
| Stream response: `nzbUrl` + `servers` | Planned | M10, after archives (ADR-0010) |
| Stream `behaviorHints.proxyHeaders` | Partial | Parsed and validated (`quirky_streams_drop_bad_sources_and_unsafe_headers`); sent by the player in M3 |
| Stream `behaviorHints.bingeGroup` | Planned | v0.x binge-watching |
| Subtitles resource | Planned | M6 |
| Subtitles in stream objects | Partial | Parsed (`stream_sources_*`); loaded into the player in M6 |
| `addon_catalog` resource | Planned | v0.x |
| Addon configuration (`config`, `configurable`) | Planned | v0.x; opens the addon's `/configure` page |
| `behaviorHints.adult` / `p2p` warnings | Partial | Parsed (M1, `quirky_manifest_*`); shown in UI in M5 |
| Response caching (`Cache-Control`) | Planned | v0.x |
| Meta `links`, `trailers` | Planned | v0.x |
| Native EPG (`epgProvider`, scheduled videos) | Planned | After v1.0 (parity goal) |

## Application features

| Feature | Status | Notes |
|---------|--------|-------|
| Install/remove/order addons | Planned | M4 (persisted) |
| Board (browsable catalogs) | Planned | M5 (egui, ADR-0011) |
| Discover with filters | Planned | M5 |
| Search across addons | Supported | `search_asks_only_catalogs_that_support_search`; UI in M5 |
| Detail page | Planned | M5 |
| Stream list aggregated across addons | Supported | `meta_falls_back_*`, partial failure kept per addon |
| Playback via external mpv | Planned | M3 |
| Embedded playback in the window | Planned | Future (libmpv render API) |
| Audio and subtitle track selection | Planned | M3 |
| Library | Planned | M4 |
| Watch progress / continue watching | Planned | M4 |
| Deep links (`cineo://`) | Planned | v0.x |
| `stremio://` addon install links | Planned | v0.x; user confirmation required |
| `stremio://` page links (board, discover, library, search, detail) | Planned | v0.x |
| Casting | Planned | Future research |
| Account / cloud sync | Unsupported | Not planned for now (owner decision 2026-10-06) |
| Data export / import | Planned | v1.0 |

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
| Cinemeta | https://v3-cinemeta.strem.io/manifest.json | 2026-10-06 | Manifest: 0 warnings. Catalogs `movie/top` with `genre`, `series/top` with `search` OK (`cineo` CLI) |
| OpenSubtitles v3 | https://opensubtitles-v3.strem.io/manifest.json | 2026-10-06 | Manifest: 0 warnings (subtitles resource not exercised yet) |
