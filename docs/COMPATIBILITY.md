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
| Stream response: `infoHash` (torrent), `fileIdx`, `sources` | Partial | M9 in progress: P2P consent, file choice and tracker handling in the core (`the_first_torrent_play_asks_for_p2p_consent_and_accepting_starts_the_engine`, `choose_file` tests); the engine (`cineo-stream`) serves a file from a local seeder with ranges (`serves_the_chosen_file_with_ranges_from_a_local_peer`); the desktop plays torrents, asks for consent and hides them when P2P is off (`the_first_torrent_play_shows_the_p2p_notice`, `turning_p2p_off_hides_torrent_streams`). Playback in mpv from a real swarm is not yet tested through the app ([manual test](DEVELOPMENT.md#torrents-m9)) |
| Stream response: archives (`rarUrls`, `zipUrls`, `7zipUrls`, `tgzUrls`, `tarUrls`) | Planned | M10 (ADR-0010) |
| Stream response: `nzbUrl` + `servers` | Planned | M10, after archives (ADR-0010) |
| Stream `behaviorHints.proxyHeaders` | Supported | Validated in the core and again by the player; sent via `http-header-fields` (`sends_typed_commands_and_reports_events`; real mpv delivered them to an HTTP server, 2026-10-06) |
| Stream `behaviorHints.bingeGroup` | Supported | Binge-watching: next video, next streams from the same addon, binge match, play on end, next-episode notice and library advance ([ADDON_PROTOCOL.md](ADDON_PROTOCOL.md#streams-planned-m2); `the_next_video_skips_specials_and_unreleased_episodes`, `an_ended_episode_plays_the_next_one_from_the_same_binge_group`, `without_binge_watching_a_finished_episode_moves_continue_watching_on`). The player's next-episode notice has no UI test yet |
| Subtitles resource | Supported | Requested from every subtitles addon with the stream's `videoHash`/`videoSize`/`filename` hints (`playing_asks_subtitle_addons_with_the_stream_hints`); listed in the Subtitles menu and loaded into mpv when picked (`addon_subtitles_are_listed_and_requested`, `real_libmpv_loads_a_subtitle_file_without_an_extension`). Not yet checked against a live subtitles addon |
| Subtitles in stream objects | Supported | Listed first in the Subtitles menu (`playing_asks_subtitle_addons_with_the_stream_hints`, `entries_list_each_url_once_and_mark_the_loaded_selected_one`) and loaded like addon subtitles |
| `addon_catalog` resource | Planned | v0.x |
| Addon configuration (`configurable`, `configurationRequired`) | Supported | A Configure button opens the addon's `/configure` page in the browser; an addon that requires configuration is not installed, and its install error offers Configure instead (`an_addon_that_needs_configuration_is_configured_not_installed`). The manifest's `config` list is not used: the addon's own page builds the configured URL. No UI test of the buttons |
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
| Stream list aggregated across addons | Supported | `meta_falls_back_*`, partial failure kept per addon. GUI: search, quality and addon filters, and sorting within each addon's list by quality, seeders or size; seeders are read from a `👤 N` count and sizes from `behaviorHints.videoSize` or a size like `4.35 GB` in the stream text, the format one public torrent addon used on 2026-10-08 (observed with curl); streams without the value go last (`seeders_and_size_come_from_the_description`, `sorting_puts_larger_values_first_and_unknown_last_keeping_ties_in_order`, `sorting_by_seeders_reorders_the_list_and_plays_the_right_stream`) |
| Stream search and filters | Supported | Cineo-only: search by name and description (every word, any case), filter by detected quality and by addon (`stream_search_and_quality_filter_narrow_the_list_and_keep_stream_indexes`, `stream_filter_needs_every_word_and_the_quality`) |
| Playback in the window (libmpv) | Partial | libmpv loaded at runtime (ADR-0014); without it, playback fails with a message. The external mpv player was removed 2026-10-07. Playback, headers, resume, stop (also before the file loads) and tracks against real libmpv 0.41 (`real_libmpv_*`, `#[ignore]`, run 2026-10-07); controls (`crates/cineo-desktop/tests/player_ui.rs`); while the file loads, the meta `logo` pulses in the middle, or the title without one, over the dimmed meta `background` (`playing_an_http_stream_records_library_and_resumes`). Video in the window seen on Linux under Wayland and X11 (XWayland), 2026-10-07, with a test build, not yet through the full app ([manual test](DEVELOPMENT.md#manual-gui-test)). Windows and macOS: untested |
| Audio and subtitle track selection | Partial | Embedded tracks: Audio and Subtitles menus in the embedded player (`track_menus_list_tracks_and_select_one`). Addon subtitles in the same menu (`addon_subtitles_are_listed_and_requested`). A preferred subtitle language in Settings (`settings_pick_a_subtitle_language`) is applied to the file's tracks through mpv `slang` (`preferred_subtitle_language_covers_every_tag`) and otherwise to the first matching addon subtitle (`auto_pick_prefers_the_file_then_the_first_addon_match`); a second subtitle language is tried after it (`auto_pick_falls_back_to_the_secondary_language`). Preferred and second audio languages go to mpv `alang` (`settings_become_mpv_options`, `real_libmpv_accepts_the_subtitle_language`) |
| Settings (stremio-core `Settings` equivalents) | Partial | Hardware decoding, seek and short seek steps, Esc leaves fullscreen, pause on minimize, audio and subtitle languages with second choices, subtitle size, font, bold, colors, opacity, outline, background, position and ASS styling, stereo downmix ("surround sound") and passthrough, torrent download/upload limits, upload on/off, peer limit and DHT (Stremio's streaming-server cache and BitTorrent profile), interface scale. The subtitle style can also be changed from the player's Subtitles menu, in a panel that opens on hovering its "Style and delay" row, and applies to the playing video at once, together with a subtitle delay for that video only (`the_subtitles_menu_changes_the_style_and_the_delay`, `a_live_style_change_back_to_the_defaults_resets_every_property`, `real_libmpv_accepts_every_live_subtitle_style_property`, run 2026-10-07 against libmpv 0.41); binge watching and the next-episode notice duration; Cineo extras: start page, watched threshold, clear watch history, reset (`every_setting_reads_back_what_it_saves`, `settings_become_mpv_options`, `subtitle_style_becomes_mpv_options_only_when_changed`, `seek_steps_come_from_settings`, `torrent_settings_shape_the_next_engine`, `the_download_limit_slows_the_torrent_down`). Not offered: streaming-server URL (ADR-0010), external player (ADR-0014), mobile/TV-only options, crash reports, Discord, frame-rate matching |
| Interface language | Supported | English and Spanish. The default follows the OS locale (`sys-locale`) and falls back to English; Settings → Interface → Language overrides it (ADR-0017). Errors and notices from core are typed and worded by the UI. Technical details inside them (HTTP status, mpv or torrent errors) and all addon text stay as received (`the_interface_follows_the_language_setting_and_the_system`, `the_interface_language_is_picked_in_settings`, `spanish_notices_wrap_the_technical_detail`, `every_locale_parses_and_has_the_same_messages_and_arguments`, `every_message_the_code_asks_for_exists`, `messages_follow_the_locale_with_plurals_and_fall_back_to_english`, `the_system_tag_picks_a_locale`) |
| Library | Supported | Items are recorded on play and persisted (`library_items_upsert_and_delete`); CLI `library --all`; GUI Library page. Explicit "add to library" without playing: not yet. Cineo extras: a heart on each Continue watching and Library card, or under the poster on the movie or show page, marks a favourite; favourites are listed newest first in their own section at the top of the Library page, and clicking one opens its movie or show page, while Continue watching and Recently played cards resume playback. A favourite that was played is also listed in Continue watching and Recently played. Favouriting an item that was never played adds it to the library, and unfavouriting it removes it again. An item counts as played once playback progresses; one that never did and is not a favourite is not kept. The X on a Continue watching card only takes the item out of that row; on a Library card it removes the item, or for a favourite forgets only its playback and keeps the favourite (`favourites_keep_the_order_they_were_added_in_and_survive_playing`, `favoriting_from_the_detail_page_adds_an_unplayed_item_and_unfavoriting_drops_it`, `a_stream_is_saved_once_it_plays_and_an_unplayed_item_is_forgotten`, `dismissing_from_continue_watching_keeps_the_item_and_its_favourite`, `removing_from_the_library_shows_no_notice`, `removing_a_favourite_from_the_library_forgets_only_its_playback`, `a_continue_watching_card_resumes_favorites_or_dismisses_the_item`, `the_library_lists_newest_favorites_first_and_unfavorites_them`, `a_favourite_never_played_has_no_remove_button`, `favourites_open_their_page_and_recently_played_resumes`, `a_watched_favourite_is_also_in_continue_watching_and_recently_played`, `the_detail_page_adds_and_removes_a_favourite`) |
| Watch progress / continue watching | Supported | Survives restarts and resumes at the saved position (`continue_watching_resumes_at_the_saved_position_after_restart`); CLI `library`; GUI "Continue watching" row. The GUI turns mpv progress events into saved progress (INFERRED from code; covered by the manual test script, not yet run). Clicking an item in Continue watching or the Library replays the last stream that actually played for it, at the saved position, without asking the addons again (`resuming_replays_the_saved_stream_at_the_saved_position`, `a_continue_watching_card_resumes_the_item`, `playable_streams_survive_saving_and_others_are_not_saved`); it opens the detail page instead when no stream is saved, when the saved torrent needs P2P that is off or not yet accepted, or with a notice when the saved stream fails before it starts playing (`a_resume_that_played_does_not_fall_back_to_the_detail_page`, `resuming_without_a_saved_stream_opens_the_detail_page`, `a_saved_torrent_opens_the_detail_page_when_p2p_is_off`, `a_saved_stream_that_fails_opens_the_detail_page_with_a_notice`) |
| Deep links (`cineo://`) | Supported | Same forms as `stremio://` (ADR-0018; `links_parse_into_routes_and_anything_else_is_rejected`). Opened from the command line and handed to a running window on Unix (`a_link_reaches_the_running_instance_through_a_private_socket`); the Windows pipe and scheme registration are untested; macOS links are not handled |
| `stremio://` addon install links | Supported | Mapped to `https://` only; installs after confirmation (`an_install_link_installs_nothing_until_accepted`) |
| `stremio://` page links (board, discover, library, search, detail) | Supported | `autoPlay` ignored (`links_parse_into_routes_and_anything_else_is_rejected`); no UI test of the navigation |
| Casting | Planned | Future research |
| Account / cloud sync | Unsupported | Not planned for now (owner decision 2026-10-06) |
| Data export / import | Planned | v1.0 |

## Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Linux x86_64 | Partial | CLI and GUI run (KDE Wayland, 2026-10-06); no package yet (M7) |
| Windows x86_64 | Planned | CI from M0 |
| macOS aarch64 | Planned | CI from M0; best-effort until v1.0 |
| Android / iOS / TV / Web | Unsupported | Postponed ([ROADMAP.md](ROADMAP.md#platforms)) |

## Verified addons

Real addons Cineo has been checked against. Add a row whenever you verify one.
Only list public addons, never configured URLs.

| Addon | Manifest URL | Checked | Result |
|-------|-------------|---------|--------|
| Cinemeta | https://v3-cinemeta.strem.io/manifest.json | 2026-10-06 | Manifest: 0 warnings. Catalogs `movie/top` with `genre`, `series/top` with `search` OK (`cineo` CLI) |
| OpenSubtitles v3 | https://opensubtitles-v3.strem.io/manifest.json | 2026-10-07 | Manifest: 0 warnings (2026-10-06). `subtitles/movie/tt0111161/filename=….json` returned 39 subtitles with `id`, `url`, `lang` (ISO 639-2, including `ell` and the non-standard `pob`); file URLs have no extension (observed with curl, not yet through the app) |
