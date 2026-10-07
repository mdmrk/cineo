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
| Stream response: `infoHash` (torrent), `fileIdx`, `sources` | Partial | M9 in progress: P2P consent, file choice and tracker handling in the core (`the_first_torrent_play_asks_for_p2p_consent_and_accepting_starts_the_engine`, `choose_file` tests); the engine (`cineo-stream`) serves a file from a local seeder with ranges (`serves_the_chosen_file_with_ranges_from_a_local_peer`); the desktop plays torrents, asks for consent and hides them when P2P is off (`the_first_torrent_play_shows_the_p2p_notice`, `turning_p2p_off_hides_torrent_streams`). Playback in mpv from a real swarm is not yet tested through the app ([manual test](dev/GUI_MANUAL_TEST.md#torrents-m9)) |
| Stream response: archives (`rarUrls`, `zipUrls`, `7zipUrls`, `tgzUrls`, `tarUrls`) | Planned | M10 (ADR-0010) |
| Stream response: `nzbUrl` + `servers` | Planned | M10, after archives (ADR-0010) |
| Stream `behaviorHints.proxyHeaders` | Supported | Validated in the core and again by the player; sent via `http-header-fields` (`sends_typed_commands_and_reports_events`; real mpv delivered them to an HTTP server, 2026-10-06) |
| Stream `behaviorHints.bingeGroup` | Planned | v0.x binge-watching |
| Subtitles resource | Planned | M6 |
| Subtitles in stream objects | Partial | Parsed (`stream_sources_*`); loaded into the player in M6 |
| `addon_catalog` resource | Planned | v0.x |
| Addon configuration (`config`, `configurable`) | Planned | v0.x; opens the addon's `/configure` page |
| `behaviorHints.adult` / `p2p` warnings | Partial | Parsed (M1, `quirky_manifest_*`); shown as badges on the GUI Addons page (no UI test yet) |
| Response caching (`Cache-Control`) | Planned | v0.x |
| Meta `links`, `trailers` | Planned | v0.x |
| Native EPG (`epgProvider`, scheduled videos) | Planned | After v1.0 (parity goal) |

## Application features

| Feature | Status | Notes |
|---------|--------|-------|
| Install/remove/order addons | Supported | Persisted in order (`addons_round_trip_in_order_and_replace_the_previous_list`); CLI `addon add/remove/list` (`addons_are_added_listed_and_removed_across_runs`); GUI Addons page with install, move up/down and remove; the saved order holds whatever order manifests load in (`addons_keep_the_saved_order_whatever_order_they_load_in`); an addon whose manifest fails stays installed and can be retried or removed (`an_addon_that_fails_to_load_stays_installed`, `addons_that_failed_to_load_can_be_retried_or_removed`) |
| Board (browsable catalogs) | Supported | Rows fail independently (`board_shows_rows_with_independent_failures_and_a_card_opens_the_detail`); each addon's rows load as soon as its manifest does (`board_rows_load_as_soon_as_their_addon_does`); live Cinemeta board rendered on Linux, 2026-10-06 |
| Discover with filters | Supported | Genre and `skip` paging in the core (`discover_pages_with_skip_and_deduplicates`); catalog and genre pickers in the GUI. No dedicated UI test |
| Search across addons | Supported | `search_asks_only_catalogs_that_support_search`; GUI search page |
| Detail page | Supported | Meta, seasons/episodes, streams per addon; unplayable sources disabled with a reason (`detail_lists_streams_and_only_playable_ones_can_be_played`) |
| Stream list aggregated across addons | Supported | `meta_falls_back_*`, partial failure kept per addon |
| Playback via external mpv | Supported | Fallback when libmpv is unavailable, or with `--external-player`. `cineo-player-mpv` tests (fake IPC peer); IPC commands verified against real mpv 0.41 on Linux, headless, 2026-10-06. Windows named pipe: untested |
| Embedded playback in the window | Partial | libmpv loaded at runtime (ADR-0014). Playback, headers, resume, stop and tracks against real libmpv 0.41 (`real_libmpv_*`, `#[ignore]`, run 2026-10-07); controls (`crates/cineo-desktop/tests/player_ui.rs`). Video in the window seen on Linux under Wayland and X11 (XWayland), 2026-10-07, with a test build, not yet through the full app ([manual test](dev/GUI_MANUAL_TEST.md)). Windows and macOS: untested |
| Audio and subtitle track selection | Partial | Embedded tracks: Audio and Subtitles menus in the embedded player (`track_menus_list_tracks_and_select_one`); mpv's own controls in the external player. Addon subtitles: M6 |
| Library | Supported | Items are recorded on play and persisted (`library_items_upsert_and_delete`); CLI `library --all`; GUI Library page. Explicit "add to library" without playing: not yet |
| Watch progress / continue watching | Supported | Survives restarts and resumes at the saved position (`continue_watching_resumes_at_the_saved_position_after_restart`); CLI `library`; GUI "Continue watching" row. The GUI turns mpv progress events into saved progress (INFERRED from code; covered by the manual test script, not yet run) |
| Deep links (`cineo://`) | Planned | v0.x |
| `stremio://` addon install links | Planned | v0.x; user confirmation required |
| `stremio://` page links (board, discover, library, search, detail) | Planned | v0.x |
| Casting | Planned | Future research |
| Account / cloud sync | Unsupported | Not planned for now (owner decision 2026-10-06) |
| Data export / import | Planned | v1.0 |

## Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Linux x86_64 | Partial | CLI and GUI run (KDE Wayland, 2026-10-06); no package yet (M7) |
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
