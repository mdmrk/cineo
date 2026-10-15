# Compatibility

This file tracks what actually works. Nothing is claimed without evidence.

## Scope

Cineo aims for the same user-facing behavior as Stremio on the desktop
(ADR-0010): addons, browsing, streams (including torrents), playback,
library and deep links. An addon that works with the reference client
works with Cineo for the protocol surface in
[ADDON_PROTOCOL.md](ADDON_PROTOCOL.md): Cineo sends the same requests and
reads the responses the same way, including the reference client's
tolerance of malformed data.

Out of scope:
- Stremio's private account API (`api.strem.io`), its streaming server, its
  branding and its UI. Cineo matches behavior, not look.
- The legacy (`/stremio/v1`) and IPFS addon transports (ADR-0007).
- Cloud or account sync, analytics and telemetry.
- Hosting, indexing or recommending content sources. Cineo is a client.
- Platforms other than Linux x86_64.

| Status | Meaning |
|--------|---------|
| **Verified** | Automated test **and** checked against real addons or players (date in Notes) |
| **Supported** | Implemented, with automated tests |
| **Partial** | Implemented with known gaps (listed in Notes) |
| **Experimental** | Implemented, untested or behind a flag |
| **Unsupported** | Not implemented (reason in Notes) |

Rules:
- A status can only move up when tests exist. Name the test in Notes.
- When behavior changes, update this file in the same commit.
- A downgrade, for example when a real addon reveals a gap, is welcome. Do
  it immediately.

_Last reviewed: 2026-10-13._

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
| Stream response: `url` (http/https) | Supported | `stream_sources_are_recognized_in_reference_order` |
| Stream response: `ytId` | Unsupported | Listed but refused with a notice |
| Stream response: `externalUrl` | Unsupported | Listed but refused with a notice |
| Stream response: `infoHash` (torrent), `fileIdx`, `sources` | Partial | P2P consent, file choice and tracker handling in the core (`the_first_torrent_play_asks_for_p2p_consent_and_accepting_starts_the_engine`, `choose_file` tests); the engine (`cineo-stream`) serves a file from a local seeder with ranges (`serves_the_chosen_file_with_ranges_from_a_local_peer`); the desktop plays torrents, asks for consent and hides them when P2P is off (`the_first_torrent_play_shows_the_p2p_notice`, `turning_p2p_off_hides_torrent_streams`). Playback from a real swarm through the app is covered by the [manual test](DEVELOPMENT.md#torrents) only. While a torrent plays, it downloads in bursts: up to 30 s past the playhead, then nothing until mpv's buffer drops below 25 s (ADR-0023; `buffered_seconds_are_reported_when_the_whole_second_changes`, `the_torrent_fills_up_to_thirty_seconds_and_resumes_below_twenty_five`, `a_zero_lookahead_downloads_nothing_until_it_is_raised`, `a_short_lookahead_downloads_only_the_pieces_being_read`, `a_lookahead_of_one_byte_still_streams_the_whole_file`); the torrent data already held stays as a cache up to the configured size; not seen with a real swarm |
| Stream response: archives (`rarUrls`, `zipUrls`, `7zipUrls`, `tgzUrls`, `tarUrls`) | Unsupported | Listed but refused with a notice naming the source kind |
| Stream response: `nzbUrl` + `servers` | Unsupported | Listed but refused with a notice naming the source kind |
| Stream `behaviorHints.proxyHeaders` | Supported | Validated in the core and again by the player; sent via `http-header-fields` (`sends_typed_commands_and_reports_events`; real mpv delivered them to an HTTP server, 2026-10-06) |
| Stream `behaviorHints.bingeGroup` | Supported | Binge-watching: next video, next streams from the same addon, binge match, play on end, next-episode notice and library advance ([ADDON_PROTOCOL.md](ADDON_PROTOCOL.md#streams); `the_next_video_skips_specials_and_unreleased_episodes`, `an_ended_episode_plays_the_next_one_from_the_same_binge_group`, `without_binge_watching_a_finished_episode_moves_continue_watching_on`). The player's next-episode notice has no UI test |
| Subtitles resource | Supported | Requested from every subtitles addon with the stream's `videoHash`/`videoSize`/`filename` hints (`playing_asks_subtitle_addons_with_the_stream_hints`); listed in the Subtitles menu and loaded into mpv when picked (`addon_subtitles_are_listed_and_requested`, `real_libmpv_loads_a_subtitle_file_without_an_extension`). Not checked against a live subtitles addon |
| Subtitles in stream objects | Supported | Listed first in the Subtitles menu (`playing_asks_subtitle_addons_with_the_stream_hints`, `entries_list_each_url_once_and_mark_the_loaded_selected_one`) and loaded like addon subtitles |
| `addon_catalog` resource | Supported | Installed addons' `addonCatalogs` of type `all` with no required extra are listed on the Addons page when it opens; entries with an invalid transport URL or manifest, and repeated URLs, are skipped with a warning; each list is shown sorted by addon name, ignoring case (`listed_addons_are_sorted_by_name`); listed addons install through the normal install (their own manifest is fetched again) or offer Configure when they require configuration (`quirky_addon_catalog_keeps_valid_addons_and_reports_the_rest`, `addon_catalog_without_addons_is_an_error`, `addon_lists_load_when_asked_and_only_failed_ones_retry`, `addon_lists_follow_the_installed_addons`, `addon_lists_install_listed_addons_and_mark_installed_ones`, `opening_the_addons_page_loads_the_addon_lists`; live: Cinemeta `all/official` 6 of 7 listed, the legacy `/stremio/v1` entry skipped, `all/community` 96 of 96, 2026-10-10). Lists of other types are not shown |
| Addon configuration (`configurable`, `configurationRequired`) | Supported | A Configure button opens the addon's `/configure` page in the browser; an addon that requires configuration is not installed, and its install error offers Configure instead (`an_addon_that_needs_configuration_is_configured_not_installed`). The manifest's `config` list is not used: the addon's own page builds the configured URL. No UI test of the buttons |
| `behaviorHints.adult` / `p2p` warnings | Partial | Parsed (`quirky_manifest_*`); shown as badges on the GUI Addons page (no UI test) |
| Response caching (`Cache-Control`) | Unsupported | Every request goes to the addon |
| Meta `links`, `trailers` | Unsupported | Ignored |
| Native EPG (`epgProvider`, scheduled videos) | Unsupported | Ignored |

## Application features

| Feature | Status | Notes |
|---------|--------|-------|
| Install/remove/order addons | Supported | Persisted in order (`addons_round_trip_in_order_and_replace_the_previous_list`); CLI `addon add/remove/list` (`addons_are_added_listed_and_removed_across_runs`); GUI Addons page with install, move up/down and remove; a new profile starts with Cinemeta (`https://v3-cinemeta.strem.io/manifest.json`) installed, as in Stremio, and it can be removed like any other addon; profiles that already exist are not changed (`a_fresh_database_starts_with_cinemeta_and_keeps_its_removal`); the saved order holds whatever order manifests load in (`addons_keep_the_saved_order_whatever_order_they_load_in`); an addon whose manifest fails stays installed and can be retried or removed (`an_addon_that_fails_to_load_stays_installed`, `addons_that_failed_to_load_can_be_retried_or_removed`) |
| Board (browsable catalogs) | Supported | Rows fail independently (`board_shows_rows_with_independent_failures_and_a_card_opens_the_detail`); each addon's rows load as soon as its manifest does (`board_rows_load_as_soon_as_their_addon_does`); live Cinemeta board rendered on Linux, 2026-10-06 |
| Discover with filters | Supported | Genre and `skip` paging in the core (`discover_pages_with_skip_and_deduplicates`); catalog and genre pickers in the GUI. No dedicated UI test |
| Search across addons | Supported | `search_asks_only_catalogs_that_support_search`; GUI search page |
| Detail page | Supported | Meta, seasons/episodes, streams per addon; unplayable sources disabled with a reason (`detail_lists_streams_and_only_playable_ones_can_be_played`) |
| Stream list aggregated across addons | Supported | `meta_falls_back_*`, partial failure kept per addon. GUI: search, quality and addon filters, and sorting within each addon's list by quality, seeders or size; seeders are read from a `👤 N` count and sizes from `behaviorHints.videoSize` or a size like `4.35 GB` in the stream text, the format one public torrent addon used on 2026-10-08 (observed with curl); streams without the value go last (`seeders_and_size_come_from_the_description`, `sorting_puts_larger_values_first_and_unknown_last_keeping_ties_in_order`, `sorting_by_seeders_reorders_the_list_and_plays_the_right_stream`) |
| Stream search and filters | Supported | Cineo-only: search by name and description (every word, any case), filter by detected quality and by addon (`stream_search_and_quality_filter_narrow_the_list_and_keep_stream_indexes`, `stream_filter_needs_every_word_and_the_quality`) |
| Playback in the window (libmpv) | Partial | libmpv loaded at runtime (ADR-0014); without it, playback fails with a message. The external mpv player was removed 2026-10-07. Playback, headers, resume, stop (also before the file loads) and tracks against real libmpv 0.41 (`real_libmpv_*`, `#[ignore]`, run 2026-10-07); controls (`crates/cineo-desktop/tests/player_ui.rs`); until playback first starts (the file has loaded and mpv no longer waits), the meta `logo` pulses in the middle, or the title without one, over the dimmed meta `background` (`playing_an_http_stream_records_library_and_resumes`, `the_first_load_lasts_until_playback_is_no_longer_waiting`); later, while mpv waits for the network or a seek has not finished, the animated Cineo logo shows (`an_unfinished_seek_counts_as_waiting`); not seen with a real stream. Video in the window seen under Wayland and X11 (XWayland), 2026-10-07, with a test build; the full app is covered by the [manual test](DEVELOPMENT.md#manual-gui-test). A streaming stats overlay (player button or `I`) shows torrent download speed, peers and progress, and mpv's network speed, buffered seconds, resolution, codec and bitrate, decoder and dropped frames (`stats_show_torrent_and_player_figures_that_exist`, `stats_take_mpv_values_and_drop_unavailable_ones`); not seen with a real stream. mpv buffers at most 30 s ahead of the playhead (`cache-secs=30`, `mpv_buffers_at_most_thirty_seconds_ahead`); not seen with a real stream. A 2-pixel strip under the seek bar shows the cached parts of the stream (mpv's `demuxer-cache-state` `seekable-ranges`) (`cached_ranges_come_from_the_seekable_ranges`, `cached_ranges_become_fractions_of_the_duration`; real libmpv 0.41 reports them while playing over HTTP, `real_libmpv_plays_to_the_end_with_headers_and_tracks`, run 2026-10-15) |
| Audio and subtitle track selection | Partial | Embedded tracks: Audio and Subtitles menus in the embedded player (`track_menus_list_tracks_and_select_one`). Addon subtitles in the same menu (`addon_subtitles_are_listed_and_requested`). A preferred subtitle language in Settings (`settings_pick_a_subtitle_language`) is applied to the file's tracks through mpv `slang` (`preferred_subtitle_language_covers_every_tag`) and otherwise to the first matching addon subtitle (`auto_pick_prefers_the_file_then_the_first_addon_match`); a second subtitle language is tried after it (`auto_pick_falls_back_to_the_secondary_language`). Preferred and second audio languages go to mpv `alang` (`settings_become_mpv_options`, `real_libmpv_accepts_the_subtitle_language`) |
| Settings (stremio-core `Settings` equivalents) | Partial | Hardware decoding, seek and short seek steps, Esc leaves fullscreen, pause on minimize, audio and subtitle languages with second choices, subtitle size, font, bold, colors, opacity, outline, background, position and ASS styling, stereo downmix ("surround sound") and passthrough, torrent download/upload limits, upload on/off, peer limit, DHT, and torrent data in memory or on disk, bounded by a window either way (ADR-0019, ADR-0020; `memory_mode_plays_a_file_larger_than_its_window_without_disk`, `memory_mode_downloads_forgotten_pieces_again_on_a_seek_back`, `disk_mode_keeps_only_its_window_on_disk`, `torrent_data_on_disk_is_bounded_too`) (Stremio's streaming-server cache and BitTorrent profile), interface scale. The subtitle style can also be changed from the player's Subtitles menu, in a panel that opens on hovering its "Style and delay" row, and applies to the playing video at once, together with a subtitle delay for that video only (`the_subtitles_menu_changes_the_style_and_the_delay`, `a_live_style_change_back_to_the_defaults_resets_every_property`, `real_libmpv_accepts_every_live_subtitle_style_property`, run 2026-10-07 against libmpv 0.41); binge watching and the next-episode notice duration; Cineo extras: start page, watched threshold, clear watch history, reset (`every_setting_reads_back_what_it_saves`, `settings_become_mpv_options`, `subtitle_style_becomes_mpv_options_only_when_changed`, `seek_steps_come_from_settings`, `torrent_settings_shape_the_next_engine`, `the_download_limit_slows_the_torrent_down`). Not offered: streaming-server URL (ADR-0010), external player (ADR-0014), mobile/TV-only options, crash reports, Discord, frame-rate matching |
| Interface language | Supported | English, Spanish, Russian and Italian. The default follows the OS locale (`sys-locale`) and falls back to English; Settings → Interface → Language overrides it (ADR-0017). Errors and notices from core are typed and worded by the UI. Technical details inside them (HTTP status, mpv or torrent errors) and all addon text stay as received (`the_interface_follows_the_language_setting_and_the_system`, `the_interface_language_is_picked_in_settings`, `spanish_notices_wrap_the_technical_detail`, `every_locale_parses_and_has_the_same_messages_and_arguments`, `every_message_the_code_asks_for_exists`, `messages_follow_the_locale_with_plurals_and_fall_back_to_english`, `the_system_tag_picks_a_locale`, `every_locale_has_glyphs_in_the_text_fonts`) |
| Library | Supported | Items are recorded on play and persisted (`library_items_upsert_and_delete`); CLI `library --all`; GUI Library page. Explicit "add to library" without playing or favouriting: not offered. Cineo extras: a heart on each Continue watching and Library card, or under the poster on the movie or show page, marks a favourite; favourites are listed newest first in their own section at the top of the Library page, and clicking one opens its movie or show page, while Continue watching and Recently played cards resume playback. A favourite that was played is also listed in Continue watching and Recently played. Favouriting an item that was never played adds it to the library, and unfavouriting it removes it again. An item counts as played once playback progresses; one that never did and is not a favourite is not kept. The X on a Continue watching or Recently played card removes the item from both, or for a favourite forgets only its playback and keeps the favourite; Favourites cards have no X (`favourites_keep_the_order_they_were_added_in_and_survive_playing`, `favoriting_from_the_detail_page_adds_an_unplayed_item_and_unfavoriting_drops_it`, `a_stream_is_saved_once_it_plays_and_an_unplayed_item_is_forgotten`, `removing_from_the_library_shows_no_notice`, `removing_a_favourite_from_the_library_forgets_only_its_playback`, `a_continue_watching_card_resumes_favorites_or_removes_the_item`, `the_library_lists_newest_favorites_first_and_unfavorites_them`, `a_favourite_never_played_has_no_remove_button`, `favourites_open_their_page_and_recently_played_resumes`, `a_watched_favourite_is_also_in_continue_watching_and_recently_played`, `the_detail_page_adds_and_removes_a_favourite`) |
| Watch progress / continue watching | Supported | Survives restarts and resumes at the saved position (`continue_watching_resumes_at_the_saved_position_after_restart`); CLI `library`; GUI "Continue watching" row. The GUI turns mpv progress events into saved progress (INFERRED from code; covered by the manual test script). Clicking an item in Continue watching or the Library replays the last stream that actually played for it, at the saved position, without asking the addons again (`resuming_replays_the_saved_stream_at_the_saved_position`, `a_continue_watching_card_resumes_the_item`, `playable_streams_survive_saving_and_others_are_not_saved`); it opens the detail page instead when no stream is saved, when the saved torrent needs P2P that is off or not yet accepted, or with a notice when the saved stream fails before it starts playing (`a_resume_that_played_does_not_fall_back_to_the_detail_page`, `resuming_without_a_saved_stream_opens_the_detail_page`, `a_saved_torrent_opens_the_detail_page_when_p2p_is_off`, `a_saved_stream_that_fails_opens_the_detail_page_with_a_notice`) |
| Deep links (`cineo://`) | Supported | Same forms as `stremio://` (ADR-0018; `links_parse_into_routes_and_anything_else_is_rejected`). Opened from the command line and handed to a running window (`a_link_reaches_the_running_instance_through_a_private_socket`); scheme registration through `xdg-mime` has no test |
| `stremio://` addon install links | Supported | Mapped to `https://` only; installs after confirmation (`an_install_link_installs_nothing_until_accepted`) |
| `stremio://` page links (board, discover, library, search, detail) | Supported | `autoPlay` ignored (`links_parse_into_routes_and_anything_else_is_rejected`); no UI test of the navigation |
| Update notice | Supported | Cineo-only: once per start, GitHub's latest release is compared with the running version; a newer one shows as "vX.Y.Z available" under the version in the sidebar and opens its release page. Settings → About turns the check off (`restoring_settings_checks_for_an_update_once`, `a_newer_release_is_offered_and_an_older_one_is_not`, `with_update_checks_off_nothing_is_fetched_or_shown`, `the_latest_release_version_comes_from_its_tag`, `a_newer_release_shows_in_the_sidebar_and_opens_its_page`, `update_checks_can_be_turned_off_in_about`). No in-app install: the user runs the installer again |
| Casting | Unsupported | |
| Account / cloud sync | Unsupported | Out of scope (owner decision 2026-10-06) |
| Data export / import | Unsupported | Copy `cineo.db` to move between machines |

## Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Linux x86_64 | Supported | CLI and GUI run (KDE Wayland, 2026-10-06); released as a single executable with `install.sh` |
| Windows, macOS | Unsupported | Removed 2026-10-13 (ADR-0022) |
| Android / iOS / Web | Unsupported | |

## Verified addons

Real addons Cineo has been checked against. Add a row whenever you verify one.
Only list public addons, never configured URLs.

| Addon | Manifest URL | Checked | Result |
|-------|-------------|---------|--------|
| Cinemeta | https://v3-cinemeta.strem.io/manifest.json | 2026-10-06 | Manifest: 0 warnings. Catalogs `movie/top` with `genre`, `series/top` with `search` OK (`cineo` CLI) |
| OpenSubtitles v3 | https://opensubtitles-v3.strem.io/manifest.json | 2026-10-07 | Manifest: 0 warnings (2026-10-06). `subtitles/movie/tt0111161/filename=….json` returned 39 subtitles with `id`, `url`, `lang` (ISO 639-2, including `ell` and the non-standard `pob`); file URLs have no extension (observed with curl, not yet through the app) |
