# Security model

Cineo is a media client that **consumes untrusted remote data by design**:
addons are arbitrary third-party servers. This document defines the threat
model and the mandatory controls.

Sources consulted (2026-10-06):

- OWASP SSRF Prevention Cheat Sheet.
- The mpv manual and libmpv headers (client API, protocols, unsafe playlists).
- The stremio-shell-ng source (an anti-pattern: raw `mpv-command` passthrough).
- The reqwest 0.13 API (custom DNS resolver, redirect policy).

## Assets

- The user's machine and local network: routers, NAS, admin panels on
  `192.168.x.x`, cloud metadata endpoints.
- Addon transport URLs. They often embed **API keys and configuration**.
- Library and watch history (private).

## Threat actors

1. **A malicious or compromised addon.** It controls every byte of its
   responses: URLs, sizes, encodings, redirects, header values, timings.
2. **A network attacker.** This applies to `http://` addons only.
3. **A malicious media or subtitle file** reaching the player.

## Untrusted inputs

These are all treated as hostile:

- Addon URLs, manifests and JSON responses.
- Metadata strings and images.
- Stream, subtitle and external URLs.
- Redirect targets and HTTP headers, including `proxyHeaders` from addons.
- Downloaded files.
- Local media files.

## Controls

### Network (`cineo-net`)

| Risk                                                                                                            | Control                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SSRF / LAN attacks: an addon (or a URL inside its data) points at `127.0.0.1`, `192.168.1.1`, `169.254.169.254` | `NetPolicy` blocks non-public destinations **by default**. IP literals and `localhost` are checked before connecting. Hostnames are checked **after DNS resolution inside the resolver**: the connection only uses checked addresses, which defeats DNS rebinding. Blocked: loopback, RFC 1918, link-local, CGNAT, multicast, broadcast, documentation, benchmarking, reserved, unspecified, plus IPv6 forms embedding IPv4 (mapped, NAT64, 6to4).                               |
| Self-hosted addons on the LAN                                                                                   | Explicit opt-in only: `--allow-private-network` or the "Allow local network addresses" setting (off by default, read at startup; `private_networks_stay_blocked_unless_asked_for`). There is no per-addon trust (TECHNICAL_DEBT.md). A public addon can never cause private-network requests.                                                                                                                                                                              |
| Redirect-based bypass                                                                                           | Every redirect hop is re-validated: scheme, destination and https→http downgrade (refused). At most 5 hops.                                                                                                                                                                                                                                                                                                                                                                      |
| Proxy bypasses the resolver check                                                                               | System proxies are disabled for addon traffic (INFERRED necessity: a proxy resolves names itself).                                                                                                                                                                                                                                                                                                                                                                               |
| Scheme abuse (`file:`, `ftp:`, `data:`)                                                                         | Only `http`/`https` for addons and images.                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Oversized responses / decompression bombs                                                                       | Limit on **decoded** bytes (default 8 MiB), counted while streaming. `Content-Length` is checked early.                                                                                                                                                                                                                                                                                                                                                                          |
| Slow-loris addons                                                                                               | Total request timeout of 20 s, connect timeout of 10 s.                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Malformed JSON                                                                                                  | serde_json (recursion limit 128). Lenient field-level parsing never panics. The parsers are not fuzzed.                                                                                                                                                                                                                                                                                                                                                                          |
| Update check reveals usage to GitHub                                                                            | Once per start, on the desktop and on Android, `api.github.com/repos/mdmrk/cineo/releases/latest` is fetched through the same `AddonClient` and `NetPolicy` (1 MiB limit). It sends only the `Cineo/<version>` user agent; GitHub sees the IP address. On by default; Settings → About → "Check for updates" turns it off before the next start (`with_update_checks_off_nothing_is_fetched_or_shown`). Only the version is read from the response; the release page link is built by Cineo, not taken from it. |
| Credential leakage                                                                                              | Transport URLs with userinfo are rejected. Logs and errors include only the origin and resource path, never full addon URLs. Error messages from the HTTP stack are scrubbed of URLs.                                                                                                                                                                                                                                                                                            |

### Player (`cineo-player`, ADR-0014)

Playback runs in libmpv inside the Cineo window. There is no external mpv
process and no IPC socket.

| Risk                                                                                     | Control                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Option injection via the URL (`--script=…`)                                              | Load media with `loadfile` as an argument array (`mpv_command`, argv). Never build command lines from addon data. Never use `mpv_command_string`-style string commands. Embedded headers are added one at a time with `change-list http-header-fields append`, so a value is never parsed as a list (VERIFIED: a value with a comma arrives intact, `real_libmpv_plays_to_the_end_with_headers_and_tracks`). The title goes through the `force-media-title` property, not the `--title` option (which expands `${…}`). Resuming sets mpv's `start` option, formatted from the saved milliseconds, before `mpv_initialize`, so mpv seeks before the first frame. Live subtitle style changes use `mpv_set_property_async`, never a call that waits for the core from the render thread. |
| Dangerous mpv protocols (`edl://`, `lavfi://`, `av://`, `file://`, `memory://`, `fd://`) | Only `http`/`https` URLs from addons reach mpv, plus loopback URLs issued by `cineo-stream` (ADR-0010). Local files are allowed only when the **user** picked them.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Untrusted playlists                                                                      | Do not enable `--load-unsafe-playlists`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `ytdl` hook running an external program on addon URLs                                    | Set `ytdl=no` (and `load-scripts=no`). `ytId` streams are not played.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| User mpv config/scripts changing behavior                                                | Set `config=no`, `load-scripts=no`, `ytdl=no`, `osc=no`, `terminal=no`, `input-default-bindings=no`, `input-vo-keyboard=no`, `hwdec=vaapi,auto-safe` before `mpv_initialize`; there is no IPC server (`cineo-player`, `embedded::OPTIONS`). For loopback URLs from `cineo-stream` only, mpv also gets `demuxer-hysteresis-secs=100` and `network-timeout=0` (no read timeout on the local engine). The one per-playback option, `slang`, is built from Cineo's own language table, never from addon data. Revisit if users want their own config.                                                                                                                                                                                                                                      |
| Header injection via `proxyHeaders`                                                      | Header names must be RFC 7230 tokens, and values must not contain CR/LF/NUL. Otherwise the stream is rejected.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Raw command passthrough from the UI                                                      | The UI sends typed `PlayerCommand`s only (`embedded::PlayerCommand`: pause, seek, volume, mute, track ids, stop), formatted from numbers by `cineo-player`. There is no "send arbitrary mpv command" path, unlike shell-ng.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Track names from the media file                                                          | Shown as plain text, one line, at most 60 characters. The same applies to addon subtitle languages and labels in the Subtitles menu.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Loading a planted libmpv                                                                 | libmpv is looked up next to the executable first (on macOS also in the bundle's `Contents/Frameworks`), then on the system's library path, and on macOS last in `/opt/homebrew/lib` and `/usr/local/lib` (`embedded::ffi`). Whoever can write the install directory already controls the executable.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Tampered downloads                                                                       | `install.sh` checks every file against the release's `SHA256SUMS` before installing anything; release files also have build-provenance attestations. Checksums from the same release protect against corrupted or swapped mirror files, not against a compromised release.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |

### Subtitles

Subtitle parsing and rendering is delegated to mpv/libass. Cineo does not
parse subtitle formats itself (INFERRED: this reduces our attack surface; we
rely on mpv updates).

mpv never sees an addon subtitle URL. When the user picks an addon subtitle:

- Cineo fetches it through `cineo-net` under the `NetPolicy` (scheme, public
  addresses only unless private networks are allowed, redirect checks, the
  body size cap) (`subtitle_files_are_fetched_under_the_policy`). It is
  fetched only when picked, not for every listed subtitle.
- The bytes go to a file in `<cache dir>/subtitles/`, named by a counter
  (never by addon data). The directory is `0700` and the file `0600` on Unix
  (`files_are_private_and_deleted_on_drop`).
- mpv loads it with an argument-array `sub-add <absolute path> cached
  <title> <lang>`. Only absolute local paths are accepted, so the value is
  never read as a protocol (`subtitle_files_are_added_as_one_argument_each`).
  `title` and `lang` come from the addon. They are single argv elements,
  like the `loadfile` URL, and are only shown in the track list.
- The files are deleted when the playback ends, and leftovers from a crash
  are deleted when the next playback starts.

### Deep links (ADR-0018)

- `stremio://` and `cineo://` links come from web pages and other apps, so
  they are untrusted. They are parsed into an allowlist of typed routes;
  anything else is rejected.
- A link never installs an addon, plays media or changes settings without an
  explicit user confirmation that shows the full target (for example, the
  addon's origin and name).
- An addon install link maps only to `https://` and then goes through the
  normal network policy.
- An addon's Configure page opens in the system browser only on a click,
  and only for its own `http(s)` transport URL.
- Implemented (ADR-0018): `parse_link` allowlists the routes; an install
  link only sets a prompt that shows the full manifest URL
  (`an_install_link_installs_nothing_until_accepted`).
- A running Cineo accepts links from new launches on
  `<data dir>/instance/instance.sock` (mode 0600, in a directory of mode
  0700, `a_link_reaches_the_running_instance_through_a_private_socket`), or
  on Windows on a loopback port (ADR-0032). The Windows port is reachable by
  any local process, but a request must first present the 64-digit random
  token in `<data dir>\instance\instance.key`, so any process that can read
  the user's profile can send a link. A request of at most 16 KiB is read,
  and it is handled like any other link. The one other request, `raise`,
  only brings the window forward.
- Registering Cineo for `stremio://` and `cineo://` is opt-in from Settings
  and per user only.

### Streaming engine (ADR-0010)

| Risk                                                                                          | Control                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Other local users or web pages reach the engine's HTTP server (as with any localhost service) | binds `127.0.0.1` only; a random 128-bit per-session path token, compared in constant time; `GET`/`HEAD` only; no CORS headers; the `Host` header must be `127.0.0.1:<port>` or `localhost:<port>` (DNS-rebinding defense). Test: `requests_without_the_token_or_from_a_foreign_host_are_refused`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Disk exhaustion                                                                               | only the torrent being played is on disk. Its data is deleted when it stops, and data left by a crash is deleted when the engine starts (`stopping_deletes_the_torrent_data`, `leftover_torrent_data_is_deleted_at_start`, `leftover_torrents_are_deleted_and_nothing_else`). "Torrent data" keeps only a window around the playback position: on disk 1 to 8 GB (the default is 1 GB on the desktop and 4 GB on Android; `disk_mode_keeps_only_its_window_on_disk`, `torrent_data_is_on_disk_with_1_gb_by_default`, ADR-0020), or in memory (ADR-0019), where nothing of the torrent is written to disk (`memory_mode_plays_a_file_larger_than_its_window_without_disk`). Either can go over the window by the pieces in flight and partial pieces left by dead peers (TECHNICAL_DEBT.md)                                                                                                                                                                                                              |
| Path traversal via torrent file names                                                         | File names from torrents never become filesystem paths. Torrent storage is `<cache>/<info hash>/<piece index>` (`downloaded_data_is_stored_by_hash_and_piece_index_only`, ADR-0020); the cache directory is `0700`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| P2P exposure (IP visible to peers, uploads)                                                   | a notice before the first torrent plays, saying the IP is visible and that Cineo uploads (`the_first_torrent_play_shows_the_p2p_notice`); a Settings switch that hides and blocks torrent streams (`turning_p2p_off_hides_torrent_streams`, `settings_switch_p2p_off`); `behaviorHints.p2p` labels on installed addons. The engine starts only when a torrent is played (by construction in the desktop shell; no test)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Trackers and DHT contacting private networks                                                  | HTTP trackers go through a loopback SOCKS5 proxy with a random per-session password that refuses non-public addresses unless private networks are allowed (`non_public_targets_are_refused_by_default`). Peers connect directly over TCP and uTP (ADR-0026); unless private networks are allowed, librqbit drops peer addresses in the same non-public ranges, written to `<cache>/blocklist.txt`, before asking them for metadata and before connecting, and refuses incoming peers from them (`loopback_peers_are_blocked_unless_private_networks_are_allowed`, `the_p2p_list_has_every_blocked_range_and_only_those`). Tracker URLs on a literal non-public IP are dropped (`trackers_on_non_public_addresses_are_dropped_by_default`). No UPnP or local service discovery. **Gap:** UDP trackers by host name (the built-in fallback trackers included) and the DHT are not filtered (TECHNICAL_DEBT.md) |
| Listening port                                                                                | While a torrent engine runs, it listens for peers on TCP and uTP on all interfaces, on the port from Settings (default 42713), or a random one if that port is taken (`the_engine_accepts_peers_on_its_port`, `a_taken_port_falls_back_to_a_random_one`). Only peers can use it: BitTorrent handshakes for the torrent being played. No UPnP, so the router stays closed unless the user forwards the port (ADR-0026)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

### Filesystem and persistence

- All paths are derived from the XDG data and cache directories. Addon data never
  forms a path; ids are stored as data, never used as file names.
- The desktop keeps downloaded images in `<cache dir>/images` (mode 0700,
  files 0600, `cached_images_are_private`), named by a hash of the URL so no
  addon data or token forms a path
  (`an_image_comes_back_under_a_name_that_hides_its_url`). They are fetched
  through `cineo-net`, saved only after decoding, and decoded again with the
  same limits on every read (`what_does_not_decode_is_not_saved`,
  `a_corrupt_file_is_replaced_from_the_network`). A cached image is shown
  without a request, so it stays visible after its address stops being
  allowed. Entries go after 30 days or beyond 256 MiB.
- SQLite queries use bound parameters only (`hostile_ids_are_stored_as_data`).
- The data directory is created with mode `0700`, because the
  library and the addon URLs are private (`data_directory_is_private_to_the_user`).
- A database that is corrupt or has a newer schema is never written to or
  replaced; the user is told (`corrupt_file_is_reported_and_left_untouched`,
  `newer_schema_is_refused_and_left_untouched`).
- Each library item keeps the stream it was last played from, so it can
  resume without asking the addons again. That stream's URL and
  `proxyHeaders` can carry addon tokens; they are stored in the private
  database only, never logged. A saved stream is parsed back with the same
  lenient parser as an addon response, and only playable sources are kept
  (`playable_streams_survive_saving_and_others_are_not_saved`,
  `an_unreadable_saved_stream_is_dropped_and_the_item_kept`).

### Process execution

No shell is ever invoked with addon-controlled input. Opening `externalUrl`
in the browser requires a user confirmation and an `http(s)` scheme.

### Desktop UI (ADR-0011)

- Images are fetched through `cineo-net` with the same network policy and a
  4 MiB size cap. No other image loader (egui's `http` or `file`) is
  compiled in; only `http(s)` image URIs are loaded.
- Decoders are limited to jpeg, png and webp. The image header must declare
  at most 4096 pixels per side, checked before decoding
  (`oversized_images_are_rejected_before_decoding`,
  `non_images_and_disabled_formats_are_rejected`).
- The addon list shows only the addon's host, because the full URL can
  carry configuration (`addons_page_installs_from_the_typed_url_and_lists_installed_addons`).
- Addon strings are rendered as plain text, never interpreted as markup or
  links without a scheme check.
- Addon lists (`addon_catalog`, e.g. Cinemeta's "Community") come from third
  parties and are not reviewed. The Addons page says so above the lists.
  Installing from a list takes the same path as a typed URL: the addon's own
  manifest is fetched under the network policy and validated; the listed
  manifest is never trusted (`addon_lists_install_listed_addons_and_mark_installed_ones`).
- While a video plays, Cineo serves MPRIS (`org.mpris.MediaPlayer2.cineo.instance<pid>`)
  on the session D-Bus, so media keys and the desktop's media controls
  work. Any process in the user's session can then play, pause, stop or seek
  the video, change its volume, raise the window, and read its title,
  position and length. Stream
  URLs and file paths are not exposed. The service goes away when playback
  ends.
- While a video plays, the window title names it, so window lists, task
  switchers and screen sharing can show it.
- While a video plays, Cineo asks the session D-Bus
  `org.freedesktop.ScreenSaver` to keep the screen awake, and the desktop
  portal or logind to block idle sleep.
- At start, Cineo reads the installed font files and runs `fc-match` to pick
  fallback fonts and read the desktop's font settings. If no OpenGL window can
  open it runs `zenity` or `kdialog`, when installed, to show why.
- A panic appends one line to `<data dir>/panic.log` (mode 0600 on Unix):
  time, version, thread and source location, never the panic's message
  (`a_panic_is_recorded_without_its_message`). The default hook still prints
  the message to stderr.
- Each run writes `<data dir>/cineo.log` (mode 0600 on Unix) and keeps one
  earlier run (`each_run_starts_a_log_and_keeps_the_one_before`). At the
  default level it holds Cineo's warnings and other crates' errors, without
  the info spans that name resources and info hashes
  (`by_default_the_file_keeps_cineo_warnings_and_nothing_about_what_is_watched`,
  `by_default_other_crates_only_reach_the_file_with_errors`). `-v`, `-vv` or
  `RUST_LOG` add them; such a log shows what was watched.
- On Windows the media controls are the System Media Transport Controls
  of a hidden window Cineo creates for them at the first playback: they show
  the title and accept play, pause, stop, seek and volume, and show as
  stopped after it.
  The screen is kept awake with `SetThreadExecutionState`, and links are
  registered under `HKCU\Software\Classes` with `reg.exe` (no
  administrator rights).
- The Windows release embeds `libmpv-2.dll` from a third-party build
  (zhongfly/mpv-winbuild), pinned by SHA-256 in
  `scripts/libmpv-windows.env` and checked before every build. It is
  written to `<cache dir>\libmpv\<version>\` and replaced when its size
  differs. A `libmpv-2.dll` next to the executable is loaded first, so
  anyone who can write there can run code as Cineo.

- The macOS release carries libmpv and its libraries from a third-party
  build (media-kit/libmpv-darwin-build), pinned by SHA-256 in
  `scripts/libmpv-macos.env` and checked before every build, inside
  `Cineo.app/Contents/Frameworks`. The app and the libraries are signed
  ad hoc, not with a Developer ID, and not notarized: Gatekeeper cannot
  tell who built them. Without a bundled copy, `/opt/homebrew/lib` and
  `/usr/local/lib` (Homebrew on Intel) are searched after the system path. On macOS the media controls are Now
  Playing, and the screen is kept awake with an IOKit assertion.

### Code and CI rules

- Addon HTTP goes only through `cineo-net`'s `AddonClient`/`NetPolicy`. Never
  build a second `reqwest::Client`, enable system proxies, or relax a default
  except through an explicit user-facing option.
- Never put full addon/transport URLs, stream URLs with tokens or
  `proxyHeaders` values into logs or error messages; use origin + path.
- Never pass addon strings as mpv options or command-line arguments.
- GitHub workflows and the local actions in `.github/actions/`: reference
  actions by an exact version tag (`@vX.Y.Z`);
  default `permissions: contents: read`; `persist-credentials: false`; pass
  inputs through `env:`, never by interpolating them into `run:`.
- Build caches (`Swatinem/rust-cache`) are written only by runs on `main`.
  The prerelease build reuses `main`'s cache, but a tag build never
  restores one, so a release binary is always built from a clean target
  directory.
- The Android libmpv build is cached with `actions/cache/save`, keyed by the
  hash of `scripts/build-libmpv-android.sh` and its `.env`, and saved only
  by `build (android)` on pushes to `main`, which builds it from the pinned,
  hash-checked sources. Tag builds restore that cache and rebuild libmpv
  only when no cache matches the key (ADR-0029, amended 2026-11-13), so a
  release APK can carry a libmpv that a run on `main` built, never one that
  a pull request or a tag built.
- `prune caches` runs after runs on `main` and is the only job with
  `actions: write`: it deletes every `v0-rust-*` and `libmpv-android-*`
  cache on `main` except the newest per key prefix, so the repository stays
  under GitHub's cache limit and the libmpv build is not evicted.
- The build jobs run alongside the checks; `publish` needs every check and
  build, so nothing is released unless all checks pass. A failed check can
  still leave build attestations for artifacts that are never published.
- The Android release key is only in repository secrets, decoded into
  the runner's temporary directory by `build (android)`, which runs on
  pushes only, never for pull requests. Pull requests build an unsigned
  debug APK. A missing secret or an empty keystore fails
  `scripts/release-android.sh`; an unsigned APK is never
  published (ADR-0029).
- Any change to a security default updates this document and adds a test
  that pins the new behavior.

## Defaults summary

| Setting                           | Default                                                                                                                                             |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Private-network access            | Off (opt-in: CLI flag or Settings, applied at startup)                                                                                              |
| Update check                      | On, once per start, to `api.github.com`; off in Settings → About                                                                                    |
| Torrent upload                    | On; can be turned off in Settings (librqbit `disable_upload`, behavior not covered by a test)                                                       |
| Torrent data                      | On disk with a 1 GB window (4 GB on Android); 1 to 8 GB on disk or 256 MB to 2 GB in memory in Settings (ADR-0019, ADR-0020)                        |
| Torrent listening port            | 42713, TCP and uTP, no UPnP; 1024 to 65535 in Settings (ADR-0026)                                                                                   |
| Max decoded body                  | 8 MiB                                                                                                                                               |
| Request timeout / connect timeout | 20 s / 10 s                                                                                                                                         |
| Max redirects                     | 5, no https→http                                                                                                                                    |
| mpv                               | libmpv in-window; no user config, scripts or ytdl; argument-array `loadfile` only; http(s) only; forward cache at most 120 s and 512 MiB (ADR-0024) |

## Unsafe code

`unsafe_code = "forbid"` workspace-wide. A crate that genuinely needs FFI
must:

1. Lift the lint for that crate only.
2. Justify it in an ADR.
3. Keep `unsafe` inside the modules that lift the lint, with a `reason`.

The exceptions are `cineo-player` (ADR-0014) and `cineo-android`
(ADR-0028). Each has its own lint table that sets `unsafe_code = "deny"`.
Only these modules allow it: `embedded::ffi` (libmpv bindings) and
`embedded::render` (the OpenGL renderer) in `cineo-player`, and `native`
(the JNI entry point for the TLS verifier) in `cineo-android`. These tables
are copies of the root `[workspace.lints]` with only `unsafe_code` changed;
a lint added to the root must be added to them too.

## Reporting a vulnerability

Do not open public issues for vulnerabilities. Use GitHub's private
vulnerability reporting on this repository ("Security" tab → "Report a
vulnerability"). We aim to acknowledge reports within 7 days. Only the
latest release is supported; fixes land on `main` and ship in the next
release.
