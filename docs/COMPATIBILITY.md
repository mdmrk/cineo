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
| Playback in the window (libmpv) | Partial | libmpv loaded at runtime, checked against libmpv 0.41; several parts not seen with a real stream. [Details](#playback) |
| Audio and subtitle track selection | Partial | Audio and Subtitles menus; preferred and second languages. [Details](#audio-and-subtitle-tracks) |
| Settings (stremio-core `Settings` equivalents) | Partial | Player, subtitle, torrent and interface settings; some Stremio options not offered. [Details](#settings) |
| Interface language | Supported | Nine languages, from the OS locale or Settings; Chinese not yet seen on screen. [Details](#interface-language) |
| Desktop text and scrolling | Partial | On egui 0.36.2; none of it seen on a real desktop. [Details](#desktop-text-and-scrolling) |
| Library | Supported | Recorded on play and persisted; favourites are a Cineo extra. [Details](#library) |
| Watch progress / continue watching | Supported | Survives restarts; resumes the last stream that played, at the saved position. [Details](#watch-progress) |
| Retry after the computer wakes | Partial | Desktop: 5 s after waking, connections are renewed and failed addons and views are asked for again (`failed_loads_are_retried_once_the_network_has_had_time_to_return`); a wake on a real desktop has no test, and a change of network without sleep is not noticed |
| Deep links (`cineo://`) | Supported | Same forms as `stremio://`, handed to a running window; scheme registration has no test. [Details](#deep-links) |
| `stremio://` addon install links | Supported | Mapped to `https://` only; installs after confirmation (`an_install_link_installs_nothing_until_accepted`) |
| `stremio://` page links (board, discover, library, search, detail) | Supported | `autoPlay` ignored (`links_parse_into_routes_and_anything_else_is_rejected`); no UI test of the navigation |
| Update notice | Supported | Once per start against GitHub's latest release; can be turned off. [Details](#update-notice) |
| Casting | Unsupported | |
| Account / cloud sync | Unsupported | Out of scope (owner decision 2026-10-06) |
| Data export / import | Unsupported | Close Cineo, then copy `cineo.db` to move between machines |

## Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Linux x86_64 | Supported | CLI and GUI run (KDE Wayland, 2026-10-06); released as a single `x86-64-v2` executable with `install.sh` |
| Windows x86_64 | Untested | Released with libmpv embedded; tests pass under wine; never run on real Windows. [Details](#windows) |
| macOS | Unsupported | Removed 2026-10-13 (ADR-0022) |
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
  `addons_that_failed_to_load_can_be_retried_or_removed`).

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
  against real libmpv 0.41 (`real_libmpv_*`, `#[ignore]`, run 2026-10-07);
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
- A streaming stats overlay (player button or `I`) shows torrent download
  speed, peers and progress, and mpv's network speed, buffered seconds,
  resolution, codec and bitrate, decoder and dropped frames
  (`stats_show_torrent_and_player_figures_that_exist`,
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
- While a video plays, the window title is `Cineo - <title>`
  (`the_window_title_names_what_is_playing`).
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
- Not offered: streaming-server URL (ADR-0010), external player (ADR-0014),
  mobile/TV-only options, crash reports, Discord, frame-rate matching.

### Interface language

- English, Spanish (Spain), Spanish (Latin America, `es-419`), Russian,
  Italian, German, French, Brazilian Portuguese (`pt-BR`) and Simplified
  Chinese (`zh-CN`).
- The default follows the OS locale (`sys-locale`): Spanish with no region
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
  `every_locale_has_the_same_messages_and_arguments`,
  `every_message_the_code_asks_for_exists`,
  `messages_follow_the_locale_and_fall_back_to_the_id`,
  `the_system_tag_picks_a_locale`,
  `every_locale_has_glyphs_in_the_text_fonts`).
- Chinese text is drawn with a CJK font installed on the system (ADR-0033);
  not yet seen on screen.

### Desktop text and scrolling

- On egui 0.36.2 (ADR-0033).
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
- No in-app install: the user runs the installer again.

### Windows

- Restored 2026-11-03 (ADR-0027); released as a single
  `cineo-x86_64-windows.exe` with libmpv embedded, and `install.ps1`.
- CI runs clippy and the tests on `windows-latest`.
- Under wine 2026-11-03 (`x86_64-pc-windows-gnu`): clippy clean; every test
  passes (the engine tests with a Windows temp path in place of the
  cross-compiled `CARGO_TARGET_TMPDIR`); the app starts, writes the bundled
  libmpv to the cache, opens the database and loads addons, catalogs and
  images; the named-pipe handover works
  (`a_link_reaches_the_running_instance_through_a_private_socket`, and by
  hand: a second launch with `stremio:///search?search=matrix` exits and the
  running window searches); scheme registration writes the `HKCU` keys for
  both schemes (checked by hand with `reg query`); data and cache go to
  `%APPDATA%\Cineo\data` and `%LOCALAPPDATA%\Cineo\cache`; the bundled
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

### Android

- Port accepted 2026-11-03 (ADR-0028), in progress.
- From 2026-11-04 every release and prerelease carries a signed
  `cineo-arm64-android.apk`, the only Android file (ADR-0029); from
  2026-11-05 it carries no libmpv source, which `libmpv.md` names upstream,
  and is about 23 MB.
- It is for arm64-v8a phones only; there is no Android TV build.
- It installs from the GitHub release, with no store.
- CI checks that the APK holds libmpv, FFmpeg and `libmpv.md` with matching
  hashes and passes `apksigner verify` (`scripts/package.sh android`); no
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
- No update check (`the_desktop_update_check_does_not_run`).
- Library: continue watching on the home screen, favourites and recently
  played on a Library tab, resume of the saved stream at its saved position,
  favouriting from the detail page and removal
  (`a_played_stream_resumes_where_it_stopped_after_a_restart`,
  `a_favourite_stays_after_a_restart_until_it_is_unmarked_or_cleared`; on
  the Redmi 9 a played item resumed at its saved position, and the Library
  tab, its menu and favourites worked, owner, 2026-11-04).
- Settings: hardware decoding, seek step, watched-at threshold,
  hide-controls delay, first and second audio and subtitle language, the
  desktop's subtitle style, the torrent settings (upload, data storage, on
  disk with 4 GB by default on Android where the desktop keeps 1 GB in
  memory, download and upload limits, peers, DHT, port;
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
  music pauses, and it pauses itself when another app takes the focus
  (`commands_are_built_from_typed_values` for the pause command; the focus
  handling has no test and was not yet seen on the phone).
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
  the desktop (`aPlayerThatIsStillLoadingOrBufferingHasNotStarted`,
  `aLoadedPlayerThatIsNotWaitingHasStarted`,
  `bufferingAfterTheStartKeepsItStarted`,
  `aNewVideoThatIsNotLoadedStartsOver`; not yet seen on the phone).
- The video pauses when the app leaves the screen or the screen turns off,
  and stays paused on coming back; a Settings switch, on by default on
  Android, turns this off
  (`leaving_the_app_pauses_the_video_unless_turned_off`; not yet seen on the
  phone).
- The player has a rotate button that turns the screen between portrait and
  landscape, also with the system's auto-rotate off, and gives the
  orientation back to the system on leaving the player
  (`aPortraitPlayerTurnsToLandscape`, `aLandscapePlayerTurnsToPortrait`; on
  the Redmi 9 with auto-rotate off it turned both ways and the orientation
  was unspecified again after leaving, 2026-11-09).
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
