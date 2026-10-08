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
- Future account credentials.

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

### Network (`cineo-net`, M1)

| Risk | Control |
|------|---------|
| SSRF / LAN attacks: an addon (or a URL inside its data) points at `127.0.0.1`, `192.168.1.1`, `169.254.169.254` | `NetPolicy` blocks non-public destinations **by default**. IP literals and `localhost` are checked before connecting. Hostnames are checked **after DNS resolution inside the resolver**: the connection only uses checked addresses, which defeats DNS rebinding. Blocked: loopback, RFC 1918, link-local, CGNAT, multicast, broadcast, documentation, benchmarking, reserved, unspecified, plus IPv6 forms embedding IPv4 (mapped, NAT64, 6to4). |
| Self-hosted addons on the LAN | Explicit opt-in only: `--allow-private-network` (CLI) or the "Allow local network addresses" setting (off by default, read at startup; `private_networks_stay_blocked_unless_asked_for`). Planned: a per-addon trust flag set at install time with a clear prompt. A public addon can never cause private-network requests. |
| Redirect-based bypass | Every redirect hop is re-validated: scheme, destination and https→http downgrade (refused). At most 5 hops. |
| Proxy bypasses the resolver check | System proxies are disabled for addon traffic (INFERRED necessity: a proxy resolves names itself). |
| Scheme abuse (`file:`, `ftp:`, `data:`) | Only `http`/`https` for addons and images. |
| Oversized responses / decompression bombs | Limit on **decoded** bytes (default 8 MiB), counted while streaming. `Content-Length` is checked early. |
| Slow-loris addons | Total request timeout of 20 s, connect timeout of 10 s. |
| Malformed JSON | serde_json (recursion limit 128). Lenient field-level parsing never panics. Fuzzing is planned for the parsers ([DEVELOPMENT.md](DEVELOPMENT.md#testing)). |
| Credential leakage | Transport URLs with userinfo are rejected. Logs and errors include only the origin and resource path, never full addon URLs. Error messages from the HTTP stack are scrubbed of URLs. |

### Player (`cineo-player`, M3; embedded since ADR-0014)

Playback runs in libmpv inside the Cineo window. There is no external mpv
process and no IPC socket.

| Risk | Control |
|------|---------|
| Option injection via the URL (`--script=…`) | Load media with `loadfile` as an argument array (`mpv_command`, argv). Never build command lines from addon data. Never use `mpv_command_string`-style string commands. Embedded headers are added one at a time with `change-list http-header-fields append`, so a value is never parsed as a list (VERIFIED: a value with a comma arrives intact, `real_libmpv_plays_to_the_end_with_headers_and_tracks`). The title goes through the `force-media-title` property, not the `--title` option (which expands `${…}`). Resuming uses a `seek` after `file-loaded`. |
| Dangerous mpv protocols (`edl://`, `lavfi://`, `av://`, `file://`, `memory://`, `fd://`) | Only `http`/`https` URLs from addons reach mpv, plus loopback URLs issued by `cineo-stream` (ADR-0010). Local files are allowed only when the **user** picked them. |
| Untrusted playlists | Do not enable `--load-unsafe-playlists`. |
| `ytdl` hook running an external program on addon URLs | Set `ytdl=no` (and `load-scripts=no`). `ytId` support (v0.x) may enable it only for URLs Cineo builds from a validated YouTube id (`[A-Za-z0-9_-]{11}`), decided by its own ADR. |
| User mpv config/scripts changing behavior | Set `config=no`, `load-scripts=no`, `ytdl=no`, `osc=no`, `terminal=no`, `input-default-bindings=no`, `input-vo-keyboard=no`, `hwdec=auto-safe` (`vaapi,auto-safe` on Linux) before `mpv_initialize`; there is no IPC server (`cineo-player`, `embedded::OPTIONS`). The one per-playback option, `slang`, is built from Cineo's own language table, never from addon data. Revisit if users want their own config. |
| Header injection via `proxyHeaders` | Header names must be RFC 7230 tokens, and values must not contain CR/LF/NUL. Otherwise the stream is rejected. |
| Raw command passthrough from the UI | The UI sends typed `PlayerCommand`s only (`embedded::PlayerCommand`: pause, seek, volume, mute, track ids, stop), formatted from numbers by `cineo-player`. There is no "send arbitrary mpv command" path, unlike shell-ng. |
| Track names from the media file | Shown as plain text, one line, at most 60 characters. The same applies to addon subtitle languages and labels in the Subtitles menu. |
| Loading a planted libmpv | libmpv is looked up next to the executable first, then on the system's library path (`embedded::ffi`). Whoever can write the install directory already controls the executable. On Windows the system search order applies to the bare names (UNKNOWN whether it should be restricted; review before the Windows release). |

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

### Deep links (v0.x)

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
- A running Cineo accepts links from new launches on `<data dir>/cineo.sock`
  (mode 0600, `a_link_reaches_the_running_instance_through_a_private_socket`)
  or, on Windows, a named pipe that rejects remote clients. One line of at
  most 8 KiB is read, and it is handled like any other link.
- Registering Cineo for `stremio://` and `cineo://` is opt-in from Settings
  and per user only.

### Streaming engine (M9/M10, ADR-0010)

| Risk | Control |
|------|---------|
| Other local users or web pages reach the engine's HTTP server (as with any localhost service) | Implemented (M9): binds `127.0.0.1` only; a random 128-bit per-session path token, compared in constant time; `GET`/`HEAD` only; no CORS headers; the `Host` header must be `127.0.0.1:<port>` or `localhost:<port>` (DNS-rebinding defense). Test: `requests_without_the_token_or_from_a_foreign_host_are_refused` |
| Disk exhaustion | Implemented (M9): only the torrent being played is on disk. Its data is deleted when it stops, and data left by a crash is deleted when the engine starts (`stopping_deletes_the_torrent_data`, `leftover_torrent_data_is_deleted_at_start`, `leftover_torrents_are_deleted_and_nothing_else`). One file larger than the free space can still fill the disk (TECHNICAL_DEBT.md) |
| Path traversal via torrent or archive file names | File names from torrents and archives never become filesystem paths. Torrent storage is `<cache>/<info hash>/<file index>` (`downloaded_data_is_stored_by_hash_and_file_index_only`); the cache directory is `0700` on Unix |
| P2P exposure (IP visible to peers, uploads) | Implemented (M9): a notice before the first torrent plays, saying the IP is visible and that Cineo uploads (`the_first_torrent_play_shows_the_p2p_notice`); a Settings switch that hides and blocks torrent streams (`turning_p2p_off_hides_torrent_streams`, `settings_switch_p2p_off`); `behaviorHints.p2p` labels on installed addons. The engine starts only when a torrent is played (by construction in the desktop shell; no test) |
| Malicious archives (bombs, huge member counts) | Member count, path length and decompressed-size limits. `fileMustInclude` patterns run with a size-limited regex engine (no backtracking) |
| NZB server credentials | Stored in the OS keyring only. Addon-supplied `servers` entries are shown to the user before use |
| Trackers and DHT contacting private networks | Peer TCP connections and HTTP trackers go through a loopback SOCKS5 proxy with a random per-session password that refuses non-public addresses unless private networks are allowed (`loopback_peers_are_blocked_unless_private_networks_are_allowed`, `non_public_targets_are_refused_by_default`). Tracker URLs on a literal non-public IP are dropped (`trackers_on_non_public_addresses_are_dropped_by_default`). No incoming connections, UPnP or local service discovery. **Gap:** UDP trackers by host name and the DHT do not go through the proxy (TECHNICAL_DEBT.md) |

### Filesystem and persistence (M4)

- All paths are derived from platform data directories. Addon data never
  forms a path; ids are stored as data, never used as file names.
- SQLite queries use bound parameters only (`hostile_ids_are_stored_as_data`).
- On Unix the data directory is created with mode `0700`, because the
  library and the addon URLs are private (`data_directory_is_private_to_the_user`).
  On Windows the per-user profile ACLs apply.
- A database that is corrupt or has a newer schema is never written to or
  replaced; the user is told (`corrupt_file_is_reported_and_left_untouched`,
  `newer_schema_is_refused_and_left_untouched`). `cineo doctor` opens it
  read-only.
- Each library item keeps the stream it was last played from, so it can
  resume without asking the addons again. That stream's URL and
  `proxyHeaders` can carry addon tokens; they are stored in the private
  database only, never logged. A saved stream is parsed back with the same
  lenient parser as an addon response, and only playable sources are kept
  (`playable_streams_survive_saving_and_others_are_not_saved`,
  `an_unreadable_saved_stream_is_dropped_and_the_item_kept`).
- Secrets (future account tokens) go to the OS keyring. They are never
  written to SQLite, logs or exports.

### Process execution

No shell is ever invoked with addon-controlled input. Opening `externalUrl`
in the browser requires a user confirmation and an `http(s)` scheme.

### Desktop UI (M5, ADR-0011)

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

### Code and CI rules

- Addon HTTP goes only through `cineo-net`'s `AddonClient`/`NetPolicy`. Never
  build a second `reqwest::Client`, enable system proxies, or relax a default
  except through an explicit user-facing option.
- Never put full addon/transport URLs, stream URLs with tokens or
  `proxyHeaders` values into logs or error messages; use origin + path.
- Never pass addon strings as mpv options or command-line arguments.
- GitHub workflows: pin actions by full commit SHA (with a version comment);
  default `permissions: contents: read`; `persist-credentials: false`; pass
  inputs through `env:`, never by interpolating them into `run:`.
- Any change to a security default updates this document and adds a test
  that pins the new behavior.

## Defaults summary

| Setting | Default |
|---------|---------|
| Private-network access | Off (opt-in: CLI flag or Settings, applied at startup) |
| Torrent upload | On; can be turned off in Settings (librqbit `disable_upload`, behavior not covered by a test) |
| Max decoded body | 8 MiB |
| Request timeout / connect timeout | 20 s / 10 s |
| Max redirects | 5, no https→http |
| mpv | libmpv in-window; no user config, scripts or ytdl; argument-array `loadfile` only; http(s) only |

## Unsafe code

`unsafe_code = "forbid"` workspace-wide. A crate that genuinely needs FFI
must:
1. Lift the lint for that crate only.
2. Justify it in an ADR.
3. Document every `unsafe` block with a `// SAFETY:` comment.

The one exception so far is `cineo-player` (ADR-0014): its own lint
table sets `unsafe_code = "deny"`, and only the modules `embedded::ffi`
(libmpv bindings) and `embedded::render` (the OpenGL renderer) allow it.

## Reporting a vulnerability

Do not open public issues for vulnerabilities. Use GitHub's private
vulnerability reporting on this repository ("Security" tab → "Report a
vulnerability"). We aim to acknowledge reports within 7 days. There is no
supported release yet, so fixes land on `main`.
