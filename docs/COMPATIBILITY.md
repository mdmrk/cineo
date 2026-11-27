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
- Stremio account sync, analytics and telemetry.
- Hosting, indexing or recommending content sources. Cineo is a client.
- Platforms other than Linux x86_64, Windows x86_64 and Android (ADR-0028).

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
| Stream response: `infoHash` (torrent), `fileIdx`, `sources` | Partial | Core, engine and desktop have tests; a real swarm only by the [manual test](DEVELOPMENT.md#torrents). [Details](#torrent-streams) |
| Stream response: archives (`rarUrls`, `zipUrls`, `7zipUrls`, `tgzUrls`, `tarUrls`) | Unsupported | Listed but refused with a notice naming the source kind |
| Stream response: `nzbUrl` + `servers` | Unsupported | Listed but refused with a notice naming the source kind |
| Stream `behaviorHints.proxyHeaders` | Supported | Validated in the core and again by the player; sent via `http-header-fields` (`quirky_streams_drop_bad_sources_and_unsafe_headers`; `real_libmpv_plays_to_the_end_with_headers_and_tracks`, opt-in: real mpv delivered them to an HTTP server, 2026-10-06) |
| Stream `behaviorHints.bingeGroup` | Supported | Next video and next streams from the same addon, play on end, next-episode notice. [Details](#binge-watching) |
| Subtitles resource | Supported | Requested from every subtitles addon with the stream's `videoHash`/`videoSize`/`filename` hints (`playing_asks_subtitle_addons_with_the_stream_hints`); listed in the Subtitles menu and loaded into mpv when picked (`addon_subtitles_are_listed_and_requested`, `real_libmpv_loads_a_subtitle_file_without_an_extension`). Not checked against a live subtitles addon |
| Subtitles in stream objects | Supported | Listed first in the Subtitles menu (`playing_asks_subtitle_addons_with_the_stream_hints`, `entries_list_each_url_once_and_mark_the_loaded_selected_one`) and loaded like addon subtitles |
| `addon_catalog` resource | Supported | Lists of type `all` on the Addons page; live with Cinemeta, 2026-10-10. [Details](#addon-lists) |
| Addon configuration (`configurable`, `configurationRequired`) | Supported | A Configure button opens the addon's `/configure` page in the browser; an addon that requires configuration is not installed, and its install error offers Configure instead (`an_addon_that_needs_configuration_is_configured_not_installed`). The manifest's `config` list is not used: the addon's own page builds the configured URL. No UI test of the buttons |
| `behaviorHints.adult` / `p2p` warnings | Partial | Parsed (`quirky_manifest_*`); shown as badges on the GUI Addons page (no UI test) |
| Response caching (`Cache-Control`) | Unsupported | Every request goes to the addon. Desktop images are kept on disk for up to 30 days, whatever the server's cache headers say (`a_fetched_image_is_saved_and_the_next_start_asks_nobody`) |
| Meta `links`, `trailers` | Unsupported | Ignored |
| Native EPG (`epgProvider`, scheduled videos) | Unsupported | Ignored |

## Application features

| Feature | Status | Notes |
|---------|--------|-------|
| Install/remove/order addons | Supported | Persisted in order; a new profile starts with Cinemeta; a failed addon stays installed. [Details](#installing-and-ordering-addons) |
| Board (browsable catalogs) | Supported | Rows fail independently (`board_shows_rows_with_independent_failures_and_a_card_opens_the_detail`); each addon's rows load as soon as its manifest does (`board_rows_load_as_soon_as_their_addon_does`); live Cinemeta board rendered on Linux, 2026-10-06 |
| Discover with filters | Supported | Genre and `skip` paging in the core (`discover_pages_with_skip_and_deduplicates`); catalog and genre pickers in the GUI. No dedicated UI test |
| Search across addons | Supported | `search_asks_only_catalogs_that_support_search`; GUI search page. The current search follows addons that load late or are removed (`an_addon_restored_late_joins_the_current_search`, `a_removed_addon_leaves_the_current_search`) |
| Detail page | Supported | Meta, seasons/episodes, streams per addon; unplayable sources disabled with a reason (`detail_lists_streams_and_only_playable_ones_can_be_played`). An open detail follows addons that load late, are removed or are reordered (`an_addon_restored_late_answers_a_detail_opened_before_it`, `a_removed_addon_leaves_the_streams_of_the_open_detail`, `moving_an_addon_reorders_the_open_streams_without_asking_again`) |
| Stream list aggregated across addons | Supported | Partial failure kept per addon; GUI search, filters and sorting by quality, seeders or size. [Details](#stream-list) |
| Stream search and filters | Supported | Cineo-only: search by name and description (every word, any case), filter by detected quality and by addon (`stream_search_and_quality_filter_narrow_the_list_and_keep_stream_indexes`, `stream_filter_needs_every_word_and_the_quality`) |
| Playback in the window (libmpv) | Partial | libmpv loaded at runtime, checked against libmpv 0.41 and 0.42; several parts not seen with a real stream. [Details](#playback) |
| Audio and subtitle track selection | Partial | Audio and Subtitles menus; preferred and second languages. [Details](#audio-and-subtitle-tracks) |
| Settings (stremio-core `Settings` equivalents) | Partial | Player, subtitle, torrent and interface settings; some Stremio options not offered. [Details](#settings) |
| Interface language | Supported | Nine languages, from the OS locale or Settings; Chinese not yet seen on screen. [Details](#interface-language) |
| Desktop text and scrolling | Partial | On egui 0.37.0; none of it seen on a real desktop. [Details](#desktop-text-and-scrolling) |
| Library | Supported | Recorded on play and persisted; favourites are a Cineo extra. [Details](#library) |
| Watch progress / continue watching | Supported | Survives restarts; resumes the last stream that played, at the saved position. [Details](#watch-progress) |
| Retry after the computer wakes | Partial | Desktop: 5 s after waking, connections are renewed and failed addons and views are asked for again (`failed_loads_are_retried_once_the_network_has_had_time_to_return`); a wake on a real desktop has no test, and a change of network without sleep is not noticed |
| Deep links (`cineo://`) | Supported | Same forms as `stremio://`, handed to a running window; scheme registration has no test. [Details](#deep-links) |
| `stremio://` addon install links | Supported | Mapped to `https://` only; installs after confirmation (`an_install_link_installs_nothing_until_accepted`) |
| `stremio://` page links (board, discover, library, search, detail) | Supported | `autoPlay` ignored (`links_parse_into_routes_and_anything_else_is_rejected`); no UI test of the navigation |
| Update notice | Supported | Once per start against GitHub's latest release; can be turned off. [Details](#update-notice) |
| Remote control from the phone | Partial | Cineo-only: the phone controls what plays in Cineo on a computer on the same network (ADR-0044); starting a title from the phone is not offered. [Details](#remote-control) |
| Casting | Unsupported | |
| Account / cloud sync | Verified | Desktop and Android: our own end-to-end encrypted sync, off by default, no accounts (ADR-0043); a desktop and a phone paired by QR code through the deployed server, 2026-11-27. Stremio account sync stays out of scope. [Details](#cloud-sync) |
| Data export / import | Unsupported | Close Cineo, then copy `cineo.db` to move between machines |
| Portable mode | Supported | Desktop: `--portable`, or a `portable.txt` file in the working directory, keeps the database, log and cache in the working directory; `--data-dir` and `--cache-dir` still win (`the_flag_or_a_marker_file_in_the_folder_makes_it_portable`; both ways run by hand on Linux, 2026-11-13) |

## Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Linux x86_64 | Supported | CLI and GUI run (KDE Wayland, 2026-10-06); released as a single `x86-64-v2` executable with `install.sh`; needs glibc 2.39 or newer (Ubuntu 24.04, Debian 13, Fedora 40 and later), checked on every build by `scripts/package.sh`; v1.5.0 to v1.9.7 need glibc 2.43 |
| Windows x86_64 | Untested | Released with libmpv embedded; tests pass under wine; never run on real Windows. [Details](#windows) |
| macOS (arm64, x86_64) | Untested | Restored 2026-11-12 (ADR-0035) as a universal `.dmg` with libmpv inside; never run on a Mac. [Details](#macos) |
| Android | Partial | In progress (ADR-0028); a signed arm64 APK in every release since 2026-11-04; arm64 phones only, gaps and parts not yet seen on a phone in the details. [Details](#android) |
| iOS / Web | Unsupported | |

## Verified addons

Real addons Cineo has been checked against. Add a row whenever you verify one.
Only list public addons, never configured URLs.

| Addon | Manifest URL | Checked | Result |
|-------|-------------|---------|--------|
| Cinemeta | https://v3-cinemeta.strem.io/manifest.json | 2026-10-06 | Manifest: 0 warnings. Catalogs `movie/top` with `genre`, `series/top` with `search` OK (`cineo` CLI) |
| OpenSubtitles v3 | https://opensubtitles-v3.strem.io/manifest.json | 2026-10-07 | Manifest: 0 warnings (2026-10-06). `subtitles/movie/tt0111161/filename=….json` returned 39 subtitles with `id`, `url`, `lang` (ISO 639-2, including `ell` and the non-standard `pob`); file URLs have no extension (observed with curl, not yet through the app) |

## Details

The full evidence for the rows above whose notes do not fit in the table.

### Torrent streams

- P2P consent, file choice and tracker handling in the core
  (`the_first_torrent_play_asks_for_p2p_consent_and_accepting_starts_the_engine`,
  `choose_file` tests); the engine (`cineo-stream`) serves a file from a
  local seeder with ranges
  (`serves_the_chosen_file_with_ranges_from_a_local_peer`); the desktop
  plays torrents, asks for consent and hides them when P2P is off
  (`the_first_torrent_play_shows_the_p2p_notice`,
  `turning_p2p_off_hides_torrent_streams`).
- Playback from a real swarm through the app is covered by the [manual
  test](DEVELOPMENT.md#torrents) only.
- While a torrent plays, it downloads in bursts: up to 120 s past the
  playhead, then nothing until mpv's buffer drops to 100 s (ADR-0023,
  ADR-0024, `buffered_seconds_are_reported_when_the_whole_second_changes`,
  `the_torrent_fills_up_to_two_minutes_and_resumes_at_100_seconds`,
  `a_zero_lookahead_downloads_nothing_until_it_is_raised`,
  `a_short_lookahead_downloads_only_the_pieces_being_read`,
  `a_lookahead_of_one_byte_still_streams_the_whole_file`); the torrent data
  already held stays as a cache up to the configured size; not seen with a
  real swarm.
- A refill is sized at the larger of the file's average bitrate and the
  bitrate of what mpv holds, measured once it holds 10 s
  (`fw-bytes` / `cache-duration` of one `demuxer-cache-state`), so a
  high-bitrate scene is filled in one go and a stream without a known
  duration fills in bursts too
  (`a_measured_bitrate_above_the_average_sizes_the_refill`,
  `without_a_duration_the_measured_bitrate_sizes_the_refill`,
  `the_buffered_bitrate_needs_ten_seconds_of_one_cache_state`; libmpv 0.41
  reported a steady rate for a 14 s WAV, 2026-11-15); not seen with a real
  swarm.
- Once mpv holds 30 s, a refill downloads at no more than twice that
  bitrate (at least 256 KiB/s, and never above the download limit of
  Settings), so it does not take the whole connection; a start, a seek or
  a buffer under 30 s downloads without that pace, and a stop or the next
  torrent lifts it
  (`a_refill_is_paced_at_twice_the_bitrate_once_30_seconds_are_held`,
  `the_pace_never_exceeds_the_download_limit_setting`,
  `a_download_pace_slows_the_torrent_down_until_the_next_open`); not seen
  with a real swarm.
- A seek gives up the pieces in flight that no stream wants any more and
  cancels their requests, so peers serve the new position at once
  instead of finishing those pieces first
  (`a_seek_abandons_the_pieces_in_flight_at_the_old_position` with two
  slow local seeders, and unit tests in the fork); not seen with a real
  swarm.
- With a window the torrent is added unpaused, so the peers found for the
  metadata start the download, and it downloads nothing until the player
  reads
  (`a_windowed_torrent_starts_live_without_downloading_before_a_read`); each
  open logs the time to the metadata, to initialization, to serving and to
  the first bytes sent.
- Peers connect over TCP and uTP and can connect to Cineo on its listening
  port (ADR-0026, `the_engine_accepts_peers_on_its_port`,
  `a_taken_port_falls_back_to_a_random_one`; uTP not covered by a test);
  eight public UDP trackers are always added to the stream's own
  (`the_fallback_trackers_are_udp_trackers_with_a_host_name`); not seen with
  a real swarm.

### Binge watching

- Binge-watching: next video, next streams from the same addon, binge match,
  play on end, next-episode notice and library advance
  ([ADDON_PROTOCOL.md](ADDON_PROTOCOL.md#streams);
  `the_next_video_skips_specials_and_unreleased_episodes`,
  `an_ended_episode_plays_the_next_one_from_the_same_binge_group`,
  `without_binge_watching_a_finished_episode_moves_continue_watching_on`).
- The player's next-episode notice has no UI test.
- Next to play/pause, a next-episode button shows only when there is a next
  episode and plays it
  (`the_next_button_shows_only_when_something_plays_next`).

### Addon lists

- Installed addons' `addonCatalogs` of type `all` with no required extra are
  listed on the Addons page when it opens; entries with an invalid transport
  URL or manifest, and repeated URLs, are skipped with a warning; each list
  is shown sorted by addon name, ignoring case
  (`listed_addons_are_sorted_by_name`); listed addons install through the
  normal install (their own manifest is fetched again) or offer Configure
  when they require configuration
  (`quirky_addon_catalog_keeps_valid_addons_and_reports_the_rest`,
  `addon_catalog_without_addons_is_an_error`,
  `addon_lists_load_when_asked_and_only_failed_ones_retry`,
  `addon_lists_follow_the_installed_addons`,
  `addon_lists_install_listed_addons_and_mark_installed_ones`,
  `opening_the_addons_page_loads_the_addon_lists`; live: Cinemeta
  `all/official` 6 of 7 listed, the legacy `/stremio/v1` entry skipped,
  `all/community` 96 of 96, 2026-10-10).
- Lists of other types are not shown.

### Installing and ordering addons

- Persisted in order
  (`addons_round_trip_in_order_and_replace_the_previous_list`); GUI
  Addons page with install, move up/down and remove; a new profile starts
  with Cinemeta (`https://v3-cinemeta.strem.io/manifest.json`) installed, as
  in Stremio, and it can be removed like any other addon; profiles that
  already exist are not changed
  (`a_fresh_database_starts_with_cinemeta_and_keeps_its_removal`); the saved
  order holds whatever order manifests load in
  (`addons_keep_the_saved_order_whatever_order_they_load_in`); an addon
  whose manifest fails stays installed and can be retried or removed
  (`an_addon_that_fails_to_load_stays_installed`,
  `addons_that_failed_to_load_can_be_retried_or_removed`). A manifest that
  fails at start is asked for once more before the "could not be loaded"
  notice, and the notice goes away once a retry loads every installed addon
  (`a_manifest_that_fails_at_start_is_asked_again_once_before_the_notice`,
  `the_addon_notice_goes_once_a_retry_loads_every_addon`); whether this
  stops the timeouts seen on Android after a while away is not yet known.

### Stream list

- `meta_falls_back_*`, partial failure kept per addon.
- GUI: search, quality and addon filters, and sorting within each addon's
  list by quality, seeders or size; seeders are read from a `👤 N` count and
  sizes from `behaviorHints.videoSize` or a size like `4.35 GB` in the
  stream text, the format one public torrent addon used on 2026-10-08
  (observed with curl), also written `4.35GB` or `4,35 GB`; streams without
  the value go last (`seeders_and_size_come_from_the_description`,
  `sizes_may_touch_their_unit_or_use_a_decimal_comma`,
  `sorting_puts_larger_values_first_and_unknown_last_keeping_ties_in_order`,
  `sorting_by_seeders_reorders_the_list_and_plays_the_right_stream`).

### Playback

- libmpv loaded at runtime (ADR-0014); without it, playback fails with a
  message.
- The external mpv player was removed 2026-10-07.
- Playback, headers, resume, stop (also before the file loads) and tracks
  against real libmpv 0.41 (`real_libmpv_*`, `#[ignore]`, run 2026-10-07)
  and 0.42.0 (all ten, run 2026-11-16 on Linux; Android and Windows bundle
  0.42.0 from then and were not run on a device);
  controls (`crates/cineo-desktop/tests/player_ui.rs`); until playback first
  starts (the file has loaded and mpv no longer waits), the meta `logo`
  pulses in the middle, or the title without one, over the dimmed meta
  `background` (`playing_an_http_stream_records_library_and_resumes`,
  `the_first_load_lasts_until_playback_is_no_longer_waiting`); later, while
  mpv waits for the network or a seek has not finished, the animated Cineo
  logo shows (`an_unfinished_seek_counts_as_waiting`); not seen with a real
  stream.
- Video in the window seen under Wayland and X11 (XWayland), 2026-10-07,
  with a test build; the full app is covered by the [manual
  test](DEVELOPMENT.md#manual-gui-test).
- A streaming stats overlay (player button or `I`, closed with its ×) lists
  torrent download speed, peers and progress, mpv's network speed and
  buffered seconds, and the video's resolution, codec and bitrate, decoder
  and dropped frames in a panel of fixed width; a graph before each speed
  and the buffer shows its last minute, and zero peers or dropped frames
  show in the warning color
  (`stats_show_torrent_and_player_figures_that_exist`,
  `stalled_peers_and_dropped_frames_are_flagged`,
  `trends_follow_each_figure_once_a_second`,
  `a_history_keeps_one_sample_a_second_for_a_minute`,
  `stats_take_mpv_values_and_drop_unavailable_ones`); not seen with a real
  stream.
- mpv buffers at most 120 s ahead of the playhead, and at most 512 MiB
  (`cache-secs=120`, `demuxer-max-bytes=512MiB`, ADR-0024), so above about
  35 Mbps it holds less than 2 minutes; not seen with a real stream.
- For torrents mpv stops reading at 120 s and resumes at 100 s
  (`demuxer-hysteresis-secs`), and the torrent downloads up to where mpv
  will read, from the desktop's torrent task rather than the UI thread
  (`the_torrent_fills_up_to_two_minutes_and_resumes_at_100_seconds`); not
  seen with a real stream.
- A 2-pixel strip under the seek bar shows the cached parts of the stream
  (mpv's `demuxer-cache-state` `seekable-ranges`)
  (`cached_ranges_come_from_the_seekable_ranges`,
  `cached_ranges_become_fractions_of_the_duration`; real libmpv 0.41 reports
  them while playing over HTTP,
  `real_libmpv_plays_to_the_end_with_headers_and_tracks`, run 2026-10-15).
- Playback speed from 0.25× to 2× in quarter steps, from the player's speed
  menu or with `[` and `]` (Backspace goes back to normal), for the playing
  video only; each video starts at normal speed
  (`the_speed_menu_and_brackets_change_the_playback_speed`,
  `speed_steps_through_the_list_and_stops_at_its_ends`,
  `commands_are_built_from_typed_values`); not seen with a real stream.
- The seek bar takes clicks up to 16 px above its line; above that a click
  pauses, as on the video (untested layout).
- After a seek or a volume change, the seek bar, time and volume keep the
  new value until mpv reports it, finishes the seek, or 2 s pass, so they do
  not jump back for a frame; relative seeks add up from the pending target
  (`a_click_on_the_seek_bar_holds_the_new_time_until_the_player_catches_up`,
  `a_volume_click_holds_the_new_level_until_the_player_reports_it`,
  `a_held_seek_ends_when_the_player_finishes_seeking_or_too_long_passes`,
  `relative_seeks_add_up_from_the_pending_target_within_the_media`); not
  seen with a real stream.
- While a video plays (loaded and not paused), the screen is kept awake
  through the session D-Bus `org.freedesktop.ScreenSaver` interface
  (`only_a_playing_video_keeps_the_screen_awake` covers when it applies; the
  D-Bus call itself is untested, see TECHNICAL_DEBT.md), and idle system
  sleep is blocked through `fastframe-power`
  (`the_system_stays_awake_only_while_on`; its effect on a real desktop is
  untested).
- The video pauses when the computer is about to sleep or the session ends
  (`a_suspend_or_session_end_pauses_and_is_answered`; not seen on a real
  suspend).
- While a video plays, or a torrent connects before it plays, the window
  title is `Cineo - <title>` (`the_window_title_names_what_is_playing`,
  `the_window_title_names_a_torrent_while_it_connects`).
- While a video plays, Cineo also serves MPRIS on the session D-Bus, so
  headphone and keyboard media keys and the desktop's media controls can
  play, pause, stop and seek it and set its volume, and they show its
  position and length, through `fastframe-now-playing` (ADR-0031; a seek
  outside the video is ignored and raising the volume unmutes;
  `media_keys_become_player_commands`,
  `repeated_play_or_pause_requests_cannot_undo_each_other`,
  `a_remote_seek_must_fit_a_video_of_known_length`,
  `raising_the_volume_from_the_desktop_unmutes`,
  `desktop_commands_map_to_remotes`,
  `seek_and_volume_commands_map_to_remotes`, `only_a_jump_counts_as_a_seek`,
  `the_desktop_sees_the_title_position_length_and_volume`,
  `a_stream_of_unknown_length_cannot_be_sought`,
  `nothing_playing_shows_as_stopped_with_no_track` and
  `each_process_keeps_its_own_bus_name_and_cannot_be_quit` cover the mapping
  and what is published; the D-Bus service itself is untested, see
  TECHNICAL_DEBT.md).
- On Windows the same controls go through SMTC and the screen is kept awake
  with `SetThreadExecutionState`; both untested.

### Audio and subtitle tracks

- Embedded tracks: Audio and Subtitles menus in the embedded player
  (`track_menus_list_tracks_and_select_one`).
- Addon subtitles in the same menu
  (`addon_subtitles_are_listed_and_requested`).
- On the desktop, `C` turns the shown subtitle off and turns the same one
  back on (`c_turns_the_shown_subtitle_off_and_the_same_one_back_on`); with
  none shown before, it picks the preferred language, else the first
  loaded subtitle
  (`c_without_a_previous_subtitle_picks_the_preferred_language`,
  `c_without_a_preferred_language_picks_the_first_subtitle`). An addon
  subtitle that was never loaded is not a candidate
  (`c_without_subtitles_sends_nothing`).
- A preferred subtitle language in Settings
  (`settings_pick_a_subtitle_language`) is applied to the file's tracks
  through mpv `slang` (`preferred_subtitle_language_covers_every_tag`) and
  otherwise to the first matching addon subtitle
  (`auto_pick_prefers_the_file_then_the_first_addon_match`); a second
  subtitle language is tried after it
  (`auto_pick_falls_back_to_the_secondary_language`).
- Preferred and second audio languages go to mpv `alang`
  (`settings_become_mpv_options`,
  `real_libmpv_accepts_the_subtitle_language`).

### Settings

- Hardware decoding, seek and short seek steps, Esc leaves fullscreen, pause
  on minimize, audio and subtitle languages with second choices, subtitle
  size, font, bold, colors, opacity, outline, background, position and ASS
  styling, stereo downmix ("surround sound") and passthrough, torrent
  download/upload limits, upload on/off, peer limit, DHT, the torrent
  listening port
  (`settings_take_a_torrent_port_and_ignore_one_out_of_range`,
  `the_torrent_port_stays_between_1024_and_65535`), and torrent data in
  memory or on disk, bounded by a window either way (ADR-0019, ADR-0020;
  `memory_mode_plays_a_file_larger_than_its_window_without_disk`,
  `memory_mode_downloads_forgotten_pieces_again_on_a_seek_back`,
  `disk_mode_keeps_only_its_window_on_disk`,
  `torrent_data_on_disk_is_bounded_too`) (Stremio's streaming-server cache
  and BitTorrent profile), interface scale.
- The subtitle style can also be changed from the player's Subtitles menu,
  in a panel that opens on hovering its "Style and delay" row, and applies
  to the playing video at once, together with a subtitle delay for that
  video only (`the_subtitles_menu_changes_the_style_and_the_delay`,
  `a_live_style_change_back_to_the_defaults_resets_every_property`,
  `real_libmpv_accepts_every_live_subtitle_style_property`, run 2026-10-07
  against libmpv 0.41); binge watching and the next-episode notice duration;
  Cineo extras: start page, watched threshold, clear watch history, reset
  (`every_setting_reads_back_what_it_saves`, `settings_become_mpv_options`,
  `subtitle_style_becomes_mpv_options_only_when_changed`,
  `seek_steps_come_from_settings`, `torrent_settings_shape_the_next_engine`,
  `the_download_limit_slows_the_torrent_down`).
- A search field at the top of Settings keeps the settings whose name or
  description contains every typed word, ignoring case and accents, under
  their section; a word naming a section keeps the whole section. Desktop
  (`searching_settings_shows_only_matching_rows_under_their_section`,
  `a_search_naming_a_section_shows_the_whole_section`,
  `a_search_without_matches_says_so`; a sync link clears the search,
  `a_sync_link_clears_a_settings_search_that_would_hide_pairing`) and
  Android
  (`everyWordMustAppearInARowIgnoringCaseAndAccents`,
  `aWordNamingAGroupKeepsTheWholeGroup`,
  `rowsWithoutTextOnlyShowWithoutASearch`; the Android screen itself not
  yet seen on a device).
- Not offered: streaming-server URL (ADR-0010), external player (ADR-0014),
  mobile/TV-only options, crash reports, Discord, frame-rate matching.

### Interface language

- English, Spanish (Spain), Spanish (Latin America, `es-419`), Russian,
  Italian, German, French, Brazilian Portuguese (`pt-BR`) and Simplified
  Chinese (`zh-CN`).
- The default follows the first of the OS's preferred languages that Cineo
  has (`fastframe-i18n`, ADR-0034): Spanish with no region
  or `ES` picks Spain, any other region Latin America; any Portuguese
  locale, `pt-PT` included, picks Brazilian Portuguese; any Chinese locale,
  `zh-TW` included, picks Simplified Chinese; anything else falls back to
  English; Settings → Interface → Language overrides it (ADR-0017).
- Errors and notices from core are typed and worded by the UI.
- Technical details inside them (HTTP status, mpv or torrent errors) and all
  addon text stay as received
  (`the_interface_follows_the_language_setting_and_the_system`,
  `the_interface_language_is_picked_in_settings`,
  `spanish_notices_wrap_the_technical_detail`,
  `every_catalog_translates_exactly_the_messages_the_code_asks_for`,
  `messages_follow_the_locale_and_fall_back_to_english`,
  `arguments_fill_their_placeholders_once`,
  `the_system_tag_picks_a_locale`,
  `every_locale_has_glyphs_in_the_text_fonts`).
- Chinese text is drawn with a CJK font installed on the system (ADR-0033);
  not yet seen on screen.

### Desktop text and scrolling

- On egui 0.37.0 (ADR-0033, ADR-0042).
- Text follows the desktop's font rendering, other scripts use installed
  fonts behind Cineo's own (`installed_fonts_come_after_cineos_own` covers
  the order), emoji are drawn in colour, and Linux touchpads scroll with
  glide.
- Changes to the desktop's font settings apply while Cineo runs, and on
  Wayland its text size multiplies the interface scale
  (`the_desktop_text_size_multiplies_the_interface_scale`).
- None of it has been seen on a real desktop.

### Library

- Items are recorded on play and persisted
  (`library_items_upsert_and_delete`); CLI `library --all`; GUI Library
  page.
- Explicit "add to library" without playing or favouriting: not offered.
- Cineo extras: a heart on each Continue watching and Library card, or under
  the poster on the movie or show page, marks a favourite; favourites are
  listed newest first in their own section at the top of the Library page,
  and clicking one opens its movie or show page, while Continue watching and
  Recently played cards resume playback.
- A favourite that was played is also listed in Continue watching and
  Recently played.
- Favouriting an item that was never played adds it to the library, and
  unfavouriting it removes it again.
- An item counts as played once playback progresses; one that never did and
  is not a favourite is not kept.
- The X on a Continue watching or Recently played card removes the item from
  both, or for a favourite forgets only its playback and keeps the
  favourite; Favourites cards have no X
  (`favourites_keep_the_order_they_were_added_in_and_survive_playing`,
  `favoriting_from_the_detail_page_adds_an_unplayed_item_and_unfavoriting_drops_it`,
  `a_stream_is_saved_once_it_plays_and_an_unplayed_item_is_forgotten`,
  `removing_from_the_library_shows_no_notice`,
  `removing_a_favourite_from_the_library_forgets_only_its_playback`,
  `a_continue_watching_card_resumes_favorites_or_removes_the_item`,
  `the_library_lists_newest_favorites_first_and_unfavorites_them`,
  `a_favourite_never_played_has_no_remove_button`,
  `favourites_open_their_page_and_recently_played_resumes`,
  `a_watched_favourite_is_also_in_continue_watching_and_recently_played`,
  `the_detail_page_adds_and_removes_a_favourite`).

### Watch progress

- Survives restarts and resumes at the saved position
  (`continue_watching_resumes_at_the_saved_position_after_restart`); CLI
  `library`; GUI "Continue watching" row.
- The GUI turns mpv progress events into saved progress (INFERRED from code;
  covered by the manual test script).
- Clicking an item in Continue watching or the Library replays the last
  stream that actually played for it, at the saved position, without asking
  the addons again
  (`resuming_replays_the_saved_stream_at_the_saved_position`,
  `a_continue_watching_card_resumes_favorites_or_removes_the_item`,
  `playable_streams_survive_saving_and_others_are_not_saved`); it opens the
  detail page instead when no stream is saved, when the saved torrent needs
  P2P that is off or not yet accepted, or with a notice when the saved
  stream fails before it starts playing
  (`a_resume_that_played_does_not_fall_back_to_the_detail_page`,
  `resuming_without_a_saved_stream_opens_the_detail_page`,
  `a_saved_torrent_opens_the_detail_page_when_p2p_is_off`,
  `a_saved_stream_that_fails_opens_the_detail_page_with_a_notice`).

### Deep links

- Same forms as `stremio://` (ADR-0018;
  `links_parse_into_routes_and_anything_else_is_rejected`).
- Opened from the command line and handed to a running window over a Unix
  socket, or a loopback port with a token on Windows, through
  `fastframe-instance` (ADR-0032;
  `a_link_reaches_the_running_instance_through_a_private_socket`); a launch
  without a link raises the running window instead of opening a second one
  (same test; that the compositor grants the focus is not tested); scheme
  registration through `xdg-mime` or `HKCU` has no test.

### Cloud sync

On the desktop and on Android (ADR-0043).

- **What syncs.** The library with progress and favourites, the installed
  addons and their order, and the settings except the device-local ones,
  such as private-network access
  (`a_second_device_gets_the_library_settings_and_addons`,
  `a_device_local_setting_never_travels`). Joining keeps the addons of
  both devices; a device that never changed a setting takes the synced
  value (`joining_keeps_the_addons_of_both_devices`,
  `values_never_synced_give_way_to_the_space_whatever_the_device_ids`).
- **Conflicts.** The last edit of each item, addon list or setting wins;
  a deletion and "Clear watch history" win over older edits
  (`a_tombstone_beats_an_older_edit_and_loses_to_a_newer_one`,
  `clearing_hides_older_items_but_not_newer_ones`,
  `two_devices_converge_after_exchanging_changes`). Two edits of the same
  item on different devices do not merge field by field.
- **When.** Edits go out a minute after the last change, progress at most
  every five minutes while playing and at once when playback stops; other
  devices' changes arrive at least every ten minutes, or with
  "Sync now"
  (`an_edit_is_pushed_once_the_changes_stop_for_a_minute`,
  `progress_pushes_at_most_every_five_minutes_while_playing`,
  `stopping_playback_syncs_at_once_and_other_edits_wait_a_minute`,
  `a_quiet_device_still_pulls_every_ten_minutes`). Nothing is pushed on
  exit: edits still waiting go out on the next start.
- **Settings → Data.** Turn on, pair with a code or a
  `cineo:///sync/<code>` link, choose the server, show the code and its QR
  code, change the key, and turn off with or without deleting the server
  data (`crates/cineo-desktop/tests/ui/sync.rs`;
  `turning_sync_on_saves_the_pairing_and_pushes_what_is_here`,
  `a_new_key_moves_everything_then_deletes_the_old_space`,
  `turning_sync_off_forgets_it_and_can_delete_the_server_data`). After a
  key change, the other devices stop with a message until they are paired
  again (`a_deleted_space_pauses_sync_until_it_is_paired_again`).
- **Android.** The same controls in Settings → Data. Two phones share
  their library and settings through a sync server, a phone pulls when it
  comes to the foreground, and the pairing survives a restart
  (`two_phones_share_their_library_and_settings_through_the_sync_server`);
  deleting the server data stops the other phone until it pairs with a new
  key (`turning_off_and_deleting_stops_the_other_phone_until_it_pairs_again`);
  a mistyped code or an `http` server turns nothing on
  (`a_bad_code_or_an_insecure_server_turns_nothing_on`). A `cineo:///sync/`
  link fills the code and opens Settings
  (`a_sync_link_gives_its_code_and_server`; on the Redmi 9 a link sent
  with `adb` opened Settings → Data with the code filled in, 2026-11-27).
  "Scan QR code" opens the camera, asking for the permission the first
  time, and a sync QR code fills the code and server the same way; other
  QR codes are ignored. The QR code is read from the camera's luminance
  plane with its row padding (`aQrCodeIsReadFromTheLuminancePlaneWithItsRowPadding`,
  `aFrameWithoutAQrCodeReadsNothing`). These tests drive the Rust half
  against the native server and the decoder on the JVM; the Compose
  screens and the camera have no test.
- **Server.** The client against the native host in tests, and the same
  test by hand against the deployed Worker
  (`a_running_server_syncs_recovers_and_deletes`, 2026-11-27). A server
  that lost data or went back to a backup gets it again
  (`the_daily_check_pushes_what_the_server_lost`,
  `a_server_that_went_back_makes_the_replica_start_over`).
- **On real devices.** On 2026-11-27 the owner turned sync on on the
  desktop, scanned its QR code with Cineo's scanner on the Redmi 9
  (Android 13, debug build), paired, and the phone synced through the
  deployed Worker: its pairing names
  `https://cineo-sync.cineo-sync-server.workers.dev/` and holds records
  pulled from it (read from the phone's database with `run-as`).
- **Not checked by hand.** Changing the key, turning off with deletion,
  and edits on both sides before a sync; whether a phone's own camera app
  opens a `cineo:` link is UNKNOWN. Known limitations are in
  [TECHNICAL_DEBT.md](TECHNICAL_DEBT.md) (no push on exit, whole-item
  conflicts, a server that withholds records, approximate rate limits).

### Remote control

A phone running Cineo controls playback in Cineo on a computer on the same
local network (ADR-0044). On by default on the desktop.

- **Desktop, Settings → Remote control.** The first start makes a key and
  listens on TCP port 18765 (another port when it is taken)
  (`the_remote_control_is_on_unless_it_was_turned_off`); the page shows
  the address, the code and a QR code; "New code" unpairs every phone;
  "Turn off" forgets the key and stays off after a restart, and "Turn on
  remote control" makes a new one
  (`crates/cineo-desktop/tests/ui/remote.rs`;
  `turning_the_remote_on_off_or_to_a_new_key_saves_and_restarts_it`). The
  key survives a restart (`a_restored_remote_key_starts_the_listener`,
  `remote_pairings_survive_a_reopen_and_turning_off_is_remembered`).
- **What the phone can do.** Play and pause, seek by 10 s or to a
  position, volume and mute, audio and subtitle tracks, subtitle delay,
  speed and stop (`phone_commands_become_player_commands`,
  `speed_and_subtitle_delay_from_the_phone_show_on_the_desktop_too`). It
  sees the title, position, length, volume and tracks, asking once a
  second while the remote screen is open
  (`the_phone_sees_what_plays_with_its_tracks`). While a torrent connects,
  only Stop works.
- **Android, Settings → Remote control.** Pair by scanning the QR code, by
  a `cineo:///remote/<code>?at=<address>` link, or by typing the address
  and code; "Open remote" shows the controls; "Forget" unpairs. A paired
  phone controls the computer and stays paired after a restart
  (`a_paired_phone_controls_the_desktop_and_stays_paired_after_a_restart`);
  public addresses and mistyped codes are refused
  (`pairing_needs_a_local_address_and_a_good_code`). The Compose screens
  have no test.
- **On real devices.** On 2026-11-27 a debug build on the Redmi 9
  (Android 13) opened a remote link from `adb`, paired, and showed
  "Nothing is playing on the computer." from the desktop build on Linux,
  through `adb reverse` to loopback. Over the LAN it showed "Cannot reach
  the computer" because the computer's `ufw` firewall dropped port 18765.
  Controlling a playing video from a real phone has not been tried.
- **Not offered.** Starting a title from the phone, finding the computer
  without the QR code (mDNS), more than one paired computer. Known
  limitations are in [TECHNICAL_DEBT.md](TECHNICAL_DEBT.md).

### Update notice

- Cineo-only: once per start, GitHub's latest release is compared with the
  running version; a newer one shows as "vX.Y.Z available" under the version
  in the sidebar and opens its release page.
- Settings → About turns the check off
  (`restoring_settings_checks_for_an_update_once`,
  `a_newer_release_is_offered_and_an_older_one_is_not`,
  `with_update_checks_off_nothing_is_fetched_or_shown`,
  `the_latest_release_version_comes_from_its_tag`,
  `a_newer_release_shows_in_the_sidebar_and_opens_its_page`,
  `update_checks_can_be_turned_off_in_about`).
- Windows (ADR-0040): a copy installed by `install.ps1`, or a portable
  `cineo.exe` (or, from before 2026-11-20, `cineo-desktop.exe`) in a
  writable folder, shows "Update to vX.Y.Z"
  instead, and a click downloads the release's executable, checksums and
  signature. The checksums must be signed by the release key for exactly
  that version and every file must match them; then the running
  executable is renamed aside, the new one takes its place and
  `licenses\` is replaced. "Restart to update" starts the new copy,
  which waits until the old one has closed
  (`installing_an_update_downloads_the_offered_release`,
  `a_copy_that_cannot_update_itself_downloads_nothing`,
  `a_finished_update_waits_for_a_restart`,
  `a_failed_update_says_why_and_can_be_tried_again`,
  `checksums_signed_for_the_offered_version_verify`,
  `checksums_signed_for_another_version_or_key_or_changed_are_refused`,
  `a_file_must_match_its_line_in_the_checksums`,
  `the_new_executable_takes_the_old_ones_place`,
  `a_failed_swap_leaves_nothing_behind`,
  `only_a_writable_installed_cineo_updates_itself`,
  `a_running_executable_can_be_swapped`,
  `the_license_texts_come_from_the_zip`,
  `release_files_download_up_to_the_policy_limit`,
  `a_copy_that_updates_itself_offers_to_install_the_update`,
  `an_update_shows_its_download_and_then_asks_for_a_restart`,
  `a_failed_update_can_be_tried_again`). An update from one published
  release to the next is UNKNOWN (not done yet).
- Linux (ADR-0041): the same update for a `cineo` (or `cineo-desktop`) in a writable
  folder, as `install.sh` installs it, with `cineo-x86_64-linux` as the
  executable; the new one is written runnable and the running one is
  swapped while it runs; the license texts are not replaced. "Restart
  to update" starts the path Cineo was launched from, because after the
  swap the running copy's own path is the replaced `cineo.old`
  (`installing_an_update_downloads_the_offered_release`,
  `the_new_executable_can_run`,
  `a_running_executable_can_be_swapped_on_linux`,
  `a_swapped_copy_finds_itself_at_the_replaced_executable_on_linux`).
  An update from one published release to the next is UNKNOWN (not done
  yet).
- On both, while the executable downloads the link shows a progress
  bar with the percent received when the server sends the length; the
  link's text wraps to the sidebar's width instead of being cut. Its
  tooltip the size
  received and the total as "12.6 MB / 30.1 MB"
  (`a_download_reports_how_much_has_arrived`,
  `the_percent_stays_within_the_total`,
  `progress_after_the_download_ended_changes_nothing`,
  `a_release_file_reports_its_progress_against_the_length`,
  `progress_is_reported_once_per_percent_or_per_mebibyte`,
  `an_update_shows_its_download_and_then_asks_for_a_restart`).
- macOS and Android: no in-app install; the user installs the new
  release.

### Windows

- Restored 2026-11-03 (ADR-0027); released as a single
  `cineo-x86_64-windows.exe` with libmpv embedded, and `install.ps1`.
- CI runs clippy and the tests on `windows-latest`.
- The executable carries the Cineo icon and its version as Windows
  resources (`winresource`), so Explorer and the Start menu shortcut can
  show them; not yet looked at on Windows (UNKNOWN).
- Under wine 2026-11-03 (`x86_64-pc-windows-gnu`): clippy clean; every test
  passes (the engine tests with a Windows temp path in place of the
  cross-compiled `CARGO_TARGET_TMPDIR`); the app starts, writes the bundled
  libmpv to the cache, opens the database and loads addons, catalogs and
  images; the named-pipe handover works
  (`a_link_reaches_the_running_instance_through_a_private_socket`, and by
  hand: a second launch with `stremio:///search?search=matrix` exits and the
  running window searches); scheme registration writes the `HKCU` keys for
  both schemes (checked by hand with `reg query`); the cache goes to
  `%LOCALAPPDATA%\Cineo\cache` (data went to `%APPDATA%\Cineo\data` when
  this was checked; since ADR-0034 it goes to `%LOCALAPPDATA%\Cineo\data`,
  not tried on Windows); the bundled
  libmpv plays over HTTP with headers, tracks, cache ranges and resume
  (`real_libmpv_plays_to_the_end_with_headers_and_tracks`,
  `real_libmpv_stop_reports_the_position_and_shuts_down`), the same as
  libmpv 0.41 on Linux; SMTC is not available in wine (`0x80040154`,
  logged).
- Not seen under wine: the window's contents and video in it.
- Nobody has run the app on real Windows yet, so playback, the link
  handover, scheme registration, SMTC media keys and keep-awake are UNKNOWN
  there (`desktop_commands_map_to_remotes` covers only the button mapping).
- The wine results for the named pipe and SMTC predate the moves to
  fastframe (ADR-0031, ADR-0032).

### macOS

- Restored 2026-11-12 (ADR-0035); released as `cineo-universal-macos.dmg`
  with `Cineo.app` (arm64 and x86_64, macOS 11 or newer), signed ad hoc and
  not notarized, with libmpv 0.36.0 and FFmpeg 6.0 from
  media-kit/libmpv-darwin-build in `Contents/Frameworks`.
- CI runs clippy and the tests on `macos-latest`; the release job checks
  that the bundled `libmpv.dylib` and its libraries load (`dlopen`).
- Library lookup order is tested on every platform
  (`the_bundled_library_comes_after_the_executable_dir_and_before_the_system`)
  and inside a bundle on macOS
  (`inside_an_app_bundle_the_frameworks_folder_is_searched_too`).
- `stremio://` and `cineo://` links are not handled on macOS, and the
  setting that registers them is hidden.
- Nobody has run the app on a Mac, so the window, playback, hardware
  decoding, Now Playing, keep-awake and the folders are UNKNOWN.

### Android

- Port accepted 2026-11-03 (ADR-0028), in progress.
- From 2026-11-04 every release and prerelease carries a signed
  `cineo-arm64-android.apk`, the only Android file (ADR-0029); from
  2026-11-05 it carries no libmpv source, which `libmpv.md` names upstream,
  and is about 23 MB.
- It is for arm64-v8a phones only; there is no Android TV build.
- It installs from the GitHub release, with no store.
- CI checks that the APK holds libmpv, FFmpeg and `libmpv.md` with matching
  hashes, passes `apksigner verify` and is signed with the key the F-Droid
  recipe allows (`scripts/package.sh android`); no
  APK from CI has been installed yet.
- `cineo-android` installs an addon, fills the home screen from its catalogs
  and restores the addons after a restart
  (`an_installed_addon_fills_the_board_and_is_back_after_a_restart`,
  `a_second_install_of_the_same_addon_is_refused`), showing an item once
  when a catalog repeats its id
  (`a_catalog_that_repeats_an_item_shows_it_once`), and loads images through
  `cineo-net` with its size limit
  (`images_come_through_the_addon_client_and_its_limits`), dropping a
  download nobody waits for any more
  (`an_image_nobody_waits_for_stops_downloading`).
- On a Redmi 9 (Android 13), 2026-11-03, a debug build showed Cinemeta's
  catalogs with posters and refused a second install of Cinemeta.
- Detail pages, stream lists and HTTP(S) playback in a `SurfaceView` with
  our LGPL libmpv
  (`a_stream_plays_on_the_surface_and_its_progress_reaches_the_library` with
  a fake player; on the Redmi 9 a 1080p H.264 stream played with MediaCodec,
  picture, sound, pause, seeking and rotation seen by the owner,
  2026-11-03).
- The player is released on its own thread, because releasing it on the
  shell thread crashed libmpv's AudioTrack output on the second exit
  (`the_player_is_released_on_its_own_thread_before_drop_returns`; on the
  Redmi 9 three plays and exits in a row with no crash, 2026-11-04).
- Update notice: once per start, as on the desktop, GitHub's latest
  release is compared with the running version; a newer one is posted once
  as an Android notification that opens its release page (asking for the
  notification permission on Android 13 and later, but not while a video
  plays or connects), and shows in Settings →
  About, where the check can be turned off
  (`restoring_settings_checks_for_an_update_once`,
  `a_newer_release_shows_with_its_page_until_checks_are_turned_off`; the
  notification has no test and was not yet seen on a phone). No in-app
  install: the new APK is installed over the old one.
- Library: continue watching on the home screen, favourites and recently
  played on a Library tab, resume of the saved stream at its saved position,
  favouriting from the detail page and removal
  (`a_played_stream_resumes_where_it_stopped_after_a_restart`,
  `a_favourite_stays_after_a_restart_until_it_is_unmarked_or_cleared`; on
  the Redmi 9 a played item resumed at its saved position, and the Library
  tab, its menu and favourites worked, owner, 2026-11-04).
- Settings: hardware decoding, seek step, watched-at threshold,
  hide-controls delay, first and second audio and subtitle language, the
  desktop's subtitle style with a preview drawn in Inter, the font the
  player uses on Android
  (`the_subtitle_preview_follows_the_style_settings`,
  `a_subtitle_background_replaces_the_outline_in_the_preview`; not yet seen
  on a phone), the torrent settings (upload, data storage, on
  disk with 4 GB by default on Android where the desktop keeps 1 GB on
  disk, download and upload limits, peers, DHT, port;
  `a_new_install_keeps_torrent_data_on_disk_until_changed_and_again_after_a_reset`),
  local network addresses, clearing the history and resetting, and an About
  section with the version and the bundled license texts
  (`the_app_reports_the_workspace_version`; the license screen has no test);
  addons can be reordered and removed
  (`changed_settings_reach_the_player_and_survive_a_restart_until_reset`,
  `subtitle_torrent_and_player_settings_change_by_key_and_reach_the_player`,
  `addons_can_be_reordered_and_removed_with_their_rows`; the new settings
  were seen on the Settings screen of the Redmi 9 but not yet tried in
  playback, 2026-11-05; on the Redmi 9 the seek buttons jumped the chosen 30
  s and the other settings and addon actions responded, owner, 2026-11-04).
- Torrent streams play through `cineo-stream` after the P2P notice, with a
  connecting screen, peers and speed, and a foreground service while a
  torrent is open
  (`a_torrent_stream_asks_once_then_plays_through_the_engine` with a local
  seeder, `going_back_while_a_torrent_connects_stops_it`; on the Redmi 9
  Sintel's WebTorrent torrent played within 2 s with picture and sound, and
  the notification appeared and went away, owner, 2026-11-04).
- In the player, a stats button opens the desktop's streaming stats: the
  torrent's speed, peers and progress, and mpv's network speed, buffer,
  resolution, codec and bitrate, decoder and dropped frames
  (`a_stream_plays_on_the_surface_and_its_progress_reaches_the_library`; not
  yet seen on the phone).
- Subtitles and Audio menus in the player list the file's tracks and, for
  subtitles, Off and the addons' subtitles, which are fetched through
  `cineo-net`, kept in a private cache folder for that video and loaded into
  mpv; the preferred subtitle language loads the first matching addon
  subtitle when the file has none, as on the desktop
  (`a_stream_plays_on_the_surface_and_its_progress_reaches_the_library`,
  `addon_subtitles_load_by_the_preferred_language_and_when_picked` with a
  fake player; not yet seen on the phone).
- Android's libass has no system fonts, so subtitles are drawn with the
  app's own Inter, which covers Latin, Greek and Cyrillic only, and the font
  choice in Settings has no effect there
  (`surface_subtitles_are_drawn_with_the_font_in_the_given_folder`,
  `real_libmpv_accepts_the_surface_font_options_over_a_chosen_font`,
  `theFontIsTheOnlyFileInTheFolderAndAnOlderCopyIsReplaced`; not yet seen on
  the phone).
- No subtitle delay, and no style changes from the player's menu, on
  Android.
- Search across the addons' search catalogs
  (`a_search_fills_rows_from_search_catalogs_and_opens_its_results`).
- An Addons tab lists installed addons, including ones that no longer load,
  which can be removed
  (`an_addon_that_no_longer_loads_is_listed_and_can_be_removed`).
- It also lists the addons of installed addon catalogs (Cinemeta's Official
  and Community) with Install, or Configure in the browser for addons that
  need it (`addon_lists_offer_installs_and_configuration_links`).
- Streams can be searched, filtered by quality or addon and sorted by addon
  order, quality, seeders or size, with quality badges, as on the desktop
  (`streams_can_be_filtered_and_sorted_like_on_the_desktop`; checked by the
  owner on the Redmi 9 with Torrentio, 2026-11-04).
- The interface is in the desktop's nine languages (English, Spanish for
  Spain and Latin America, Brazilian Portuguese, Russian, Italian, German,
  French and Simplified Chinese), picked from the system language, with no
  Language setting (`everyLocaleHasEveryStringWithTheSameArguments`; not yet
  seen on the phone).
- Binge watching as on the desktop: the player offers the next episode with
  a "Next episode" button above the seek bar and, from the chosen time
  before the end, a card that plays it after a 10 s countdown when binge
  watching is on; Settings has binge watching and the next-episode notice
  (`the_player_offers_the_next_episode_and_plays_it_on_request` with a fake
  player, which also checks that the replaced player's close does not stop
  the next episode; on the Redmi 9, 2026-11-05, with a local test addon, the
  card showed with the controls hidden, counted down and played the next
  episode over HTTP, H.264 and HEVC to HEVC 10-bit; the card and the button
  have no UI test).
- HEVC 10-bit plays with a picture: MediaTek's HEVC decoder fails on it and
  mpv falls back to software decoding, and software frames are converted to
  8-bit 4:2:0 before drawing
  (`surface_playback_starts_without_video_output_decodes_through_the_ndk_and_draws_8_bit`;
  seen on the Redmi 9 with hardware decoding on and off, 2026-11-05).
- Releasing a surface player waits until libmpv is destroyed, so the next
  video can take the surface
  (`real_libmpv_is_destroyed_before_an_ended_player_is_dropped`).
- While a video plays the app holds Android's audio focus, so other apps'
  music pauses. It pauses when another app takes the focus, plays again
  after a short loss such as a call if that loss paused it, leaves ducking
  to the system, and pauses when headphones are unplugged
  (`aShortLossPausesAndPlaysAgainWhenTheFocusReturns`,
  `aVideoThatWasPausedBeforeAShortLossStaysPaused`,
  `aSecondShortLossWhilePausedByTheFirstStillPlaysAgain`,
  `aLastingLossPausesAndGivesUpTheFocusForGood`, `duckingIsLeftToTheSystem`;
  not yet seen on the phone).
- A 2 dp strip under the seek bar shows the cached parts of the stream, as
  on the desktop (`cached_ranges_become_fractions_of_the_duration`,
  `a_stream_plays_on_the_surface_and_its_progress_reaches_the_library`).
- After a seek from the seek bar or the seek buttons, the bar and the time
  keep the new value until mpv reports it, finishes the seek, or 2 s pass,
  and relative seeks add up from the pending target, as on the desktop
  (`aDragKeepsItsValueUntilThePlayerCatchesUp`,
  `aHeldSeekEndsWhenThePlayerFinishesSeekingOrTooLongPasses`,
  `relativeSeeksAddUpFromThePendingTargetWithinTheMedia`; not yet seen on
  the phone).
- The screen stays awake only while the video is not paused, rotating during
  playback keeps the Activity and the video surface, and coming back from a
  detail page keeps each tab's scroll position (seen on the Redmi 9,
  2026-11-08; no automated test).
- Removing the app from the recent apps while a torrent is open stops the
  playback and the torrent (no test, not yet seen on the phone).
- A stream shows one loading screen, with only a back button, until the
  video has started, and only then the picture and the controls show, as on
  the desktop. mpv starts once it holds 3 s of video and, after running out,
  waits for 3 s again, and a rebuffer dims the picture under the loading
  logo once it lasts 0.4 s
  (`leaving_the_app_pauses_the_video_only_when_set_to_pause`
  for the start,
  `surface_playback_starts_without_video_output_decodes_through_the_ndk_and_draws_8_bit`
  for the options; on the Redmi 9, 2026-11-15, a local 1080p stream started
  in about 0.6 s and one served at two thirds of its bitrate played about
  7 s between waits of about 5 s).
- Settings → "When leaving the app" chooses what a playing video does:
  picture-in-picture (the default), keep playing, or pause. With
  picture-in-picture, leaving the app with Home keeps the video in a window
  shaped like the video, with rewind, play or pause and forward buttons;
  closing that window pauses, and the next episode starts inside it. In the
  first two modes the sound goes on with the screen off, and a video that
  has not started when the app is left, or that starts while the app is
  away, a connecting torrent included, loads and plays its sound without a
  picture or a window until the app is opened again; set to pause, it loads
  and waits paused
  (`leaving_the_app_pauses_the_video_only_when_set_to_pause`,
  `a_video_that_starts_while_the_app_is_away_plays_unless_set_to_pause`,
  `theWindowTakesTheShapeOfTheVideo`,
  `aShapeAndroidWouldRefuseIsNarrowedToItsLimit`,
  `theVideoIsCentredInTheScreenWithBarsOnTheLongSide`; all three modes, the
  window's buttons, closing it and an episode change inside it seen on the
  Redmi 9, Android 13, 2026-11-15; entering it below Android 12 was not
  tried; a torrent left with Home while it loaded started playing about
  20 s later and showed its picture on coming back, Redmi 9, 2026-11-15; a
  video created while the app is away not yet seen on the phone).
- Playback loads without waiting for the video surface. Without a surface
  the video track stays selected while paused, so the picture is there on
  coming back, and is deselected while playing, so nothing is decoded for a
  picture nobody sees (`only_a_command_that_resumes_playback_unpauses`; on
  the Redmi 9, 2026-11-15, a paused video showed its picture within about
  0.5 s of returning, and one playing in the background within about 1 s
  with the sound going on, at 6 to 9 % of one core in the background).
- Any playback, a connecting torrent included, holds the foreground service
  with a media notification and a media session, so the system's media
  controls and the lock screen show the title, artwork, position, rewind,
  forward and next episode, and headset buttons reach the player; while it
  is not paused the service also holds a wake lock and a Wi-Fi lock. The
  service stops when playback ends
  (`aPositionThatAdvancesWithTheClockIsNotReportedAgain`,
  `aSeekOrAStallIsReportedAgain`, `aPausedPositionHoldsUntilItMoves`,
  `aChangeOfStateOrButtonsIsReportedAgain`; the controls in the shade and on
  the lock screen, and the service, notification and locks gone after Back,
  seen on the Redmi 9, 2026-11-15; headset buttons were not tried).
- Holding a finger on the video plays it at 2× until the finger lifts
  (`commands_are_built_from_typed_values`; seen on the Redmi 9, 2026-11-15).
- The controls fade in and out, show whenever the video is paused or the
  app comes back, and stay while they are being used (seen on the Redmi 9,
  2026-11-15; no automated test).
- The player has a rotate button that turns the screen between portrait and
  landscape, also with the system's auto-rotate off, and gives the
  orientation back to the system on leaving the player
  (`aPortraitPlayerTurnsToLandscape`, `aLandscapePlayerTurnsToPortrait`; on
  the Redmi 9 with auto-rotate off it turned both ways and the orientation
  was unspecified again after leaving, 2026-11-09). From Android 16 on
  screens at least 600 dp wide, where Android ignores an app's orientation,
  the button is not shown
  (`largeScreensFromAndroid16OnCannotBeRotatedByTheApp`; no large screen
  seen).
- The app targets Android 17 (API 37). There, reaching addresses on the
  local network needs the `ACCESS_LOCAL_NETWORK` permission: turning on
  "Allow local network addresses" asks for it, and while the setting is on
  without it the row says so and offers "Allow", which opens the app's
  system settings after a refusal (no automated test; not yet seen on an
  Android 17 device. On the Redmi 9, Android 13, the app installed with
  target 37 and started, 2026-11-27).
- Whenever the app is opened or returned to with a network, and whenever a
  usable network appears while it is visible, it asks again for the addons,
  home and search rows, addon lists, detail and streams that failed, without
  a notice if an addon fails again; it opens new HTTP connections only when
  the network is a new one or the app was away for 90 s or more (not yet
  seen on the phone); an addon that recovers joins the current search and
  the open detail (`a_reset_stops_using_connections_that_went_silent`,
  `a_usable_network_asks_again_for_what_failed`,
  `a_usable_network_that_keeps_its_connections_asks_again_too`,
  `aNetworkThatBecomesValidatedDoesNotRenewAgain`,
  `aShortAbsenceAsksAgainWithoutRenewing`, `aLongAbsenceRenewsOnce`,
  `anotherNetworkAfterAShortAbsenceRenews`,
  `retry_asks_again_only_for_the_rows_that_failed`,
  `retry_asks_again_for_search_rows_that_failed`,
  `retry_asks_again_for_the_addons_whose_manifest_failed`,
  `retry_asks_again_for_a_failed_detail_and_its_failed_streams`,
  `retry_asks_again_for_addon_lists_that_failed`,
  `an_automatic_retry_that_fails_again_raises_no_notice`,
  `a_recovered_addon_joins_the_streams_of_the_open_detail`,
  `a_recovered_addon_joins_the_current_search`,
  `a_detail_without_addons_is_asked_for_once_they_recover`,
  `aNetworkAlreadyThereCountsOnceAndRepeatsAreIgnored`,
  `aNetworkThatBecomesValidatedCountsAgain`; on the Redmi 9 a detail that
  failed with Wi-Fi off loaded by itself when Wi-Fi came back with the app
  open, and an addon that stayed down raised no new notice, 2026-11-09).
