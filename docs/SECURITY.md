# Security model

Cineo is a media client that **consumes untrusted remote data by design**:
addons are arbitrary third-party servers. This document defines the threat
model and the mandatory controls.

Sources consulted (2026-10-06):
- OWASP SSRF Prevention Cheat Sheet.
- The mpv manual (JSON IPC, protocols, unsafe playlists).
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
| Self-hosted addons on the LAN | Explicit opt-in only: `--allow-private-network` (CLI) for now. Planned: a per-addon trust flag set at install time with a clear prompt. A public addon can never cause private-network requests. |
| Redirect-based bypass | Every redirect hop is re-validated: scheme, destination and https→http downgrade (refused). At most 5 hops. |
| Proxy bypasses the resolver check | System proxies are disabled for addon traffic (INFERRED necessity: a proxy resolves names itself). |
| Scheme abuse (`file:`, `ftp:`, `data:`) | Only `http`/`https` for addons and images. |
| Oversized responses / decompression bombs | Limit on **decoded** bytes (default 8 MiB), counted while streaming. `Content-Length` is checked early. |
| Slow-loris addons | Total request timeout of 20 s, connect timeout of 10 s. |
| Malformed JSON | serde_json (recursion limit 128). Lenient field-level parsing never panics. Fuzzing is planned for the parsers (TESTING.md). |
| Credential leakage | Transport URLs with userinfo are rejected. Logs and errors include only the origin and resource path, never full addon URLs. Error messages from the HTTP stack are scrubbed of URLs. |

### Player (`cineo-player-mpv`, M3)

| Risk | Control |
|------|---------|
| Option injection via the URL (`--script=…`) | Load media through the IPC `loadfile` command with a JSON array argument. Never build command lines from addon data. Never use `mpv_command_string`-style string commands. The title goes through the `force-media-title` property, not the `--title` option (which expands `${…}`). Resuming uses a `seek` after `file-loaded`. |
| Dangerous mpv protocols (`edl://`, `lavfi://`, `av://`, `file://`, `memory://`, `fd://`) | Only `http`/`https` URLs from addons reach mpv, plus loopback URLs issued by `cineo-stream` (ADR-0010). Local files are allowed only when the **user** picked them. |
| Untrusted playlists | Do not enable `--load-unsafe-playlists`. |
| `ytdl` hook running an external program on addon URLs | Start mpv with `--ytdl=no`. `ytId` support (v0.x) may enable it only for URLs Cineo builds from a validated YouTube id (`[A-Za-z0-9_-]{11}`), decided by its own ADR. |
| User mpv config/scripts changing behavior | Start mpv with `--no-config --ytdl=no --idle=once --terminal=no --hwdec=auto-safe` (implemented in `cineo-player-mpv`). Revisit if users want their own config. |
| Header injection via `proxyHeaders` | Header names must be RFC 7230 tokens, and values must not contain CR/LF/NUL. Otherwise the stream is rejected. |
| IPC socket hijack | The mpv manual states that IPC is not secure. The socket lives in `$XDG_RUNTIME_DIR/cineo-<pid>/` (or the temp dir), created with mode `0700`. Its name is `mpv-<pid>-<nanos>`, which is unique but **not** cryptographically random. The directory permissions are the protection. Windows named pipes use the same name scheme and default pipe ACLs (UNKNOWN whether those are sufficient; review before the Windows release). |
| Raw command passthrough from the UI | The UI sends typed `PlayerCommand`s only. There is no "send arbitrary mpv command" path, unlike shell-ng. |

### Subtitles

Subtitle parsing and rendering is delegated to mpv/libass. Cineo does not
parse subtitle formats itself (INFERRED: this reduces our attack surface; we
rely on mpv updates). Subtitle URLs follow the same network policy and are
fetched by mpv. Whether mpv applies our policy to them is **UNKNOWN**; this
must be resolved in M6. One option is fetching them through `cineo-net` into
a temp file.

### Deep links (v0.x)

- `stremio://` and `cineo://` links come from web pages and other apps, so
  they are untrusted. They are parsed into an allowlist of typed routes;
  anything else is rejected.
- A link never installs an addon, plays media or changes settings without an
  explicit user confirmation that shows the full target (for example, the
  addon's origin and name).
- An addon install link maps only to `https://` and then goes through the
  normal network policy.

### Streaming engine (M9/M10, ADR-0010)

| Risk | Control |
|------|---------|
| Other local users or web pages reach the engine's HTTP server (as with any localhost service) | Implemented (M9): binds `127.0.0.1` only; a random 128-bit per-session path token, compared in constant time; `GET`/`HEAD` only; no CORS headers; the `Host` header must be `127.0.0.1:<port>` or `localhost:<port>` (DNS-rebinding defense). Test: `requests_without_the_token_or_from_a_foreign_host_are_refused` |
| Disk exhaustion | Implemented (M9): a bounded cache (default 5 GiB) with least-recently-used eviction of whole torrents when one opens; the torrent being played may exceed the limit (TECHNICAL_DEBT.md). Tests: `the_least_recently_used_torrents_go_first_until_the_cache_fits`, `the_torrent_in_use_is_kept_even_over_the_limit`. Showing the size in settings is not done yet |
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
- Secrets (future account tokens) go to the OS keyring. They are never
  written to SQLite, logs or exports.

### Process execution

No shell is ever invoked with addon-controlled input. Opening `externalUrl`
in the browser requires a user confirmation and an `http(s)` scheme.

### Desktop UI (M5, ADR-0011)

- Images are fetched through `cineo-net` with the same network policy and a
  4 MiB size cap. The `egui_extras` `http`/`file` loaders are not compiled
  in; only `http(s)` image URIs are loaded.
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
| Private-network access | Off |
| Max decoded body | 8 MiB |
| Request timeout / connect timeout | 20 s / 10 s |
| Max redirects | 5, no https→http |
| mpv | `--no-config --ytdl=no`, IPC `loadfile` only, http(s) only |

## Unsafe code

`unsafe_code = "forbid"` workspace-wide. A crate that genuinely needs FFI
(for example a future libmpv embedding) must:
1. Lift the lint for that crate only.
2. Justify it in an ADR.
3. Document every `unsafe` block with a `// SAFETY:` comment.

## Reporting a vulnerability

Do not open public issues for vulnerabilities. Use GitHub's private
vulnerability reporting on this repository ("Security" tab → "Report a
vulnerability"). We aim to acknowledge reports within 7 days. There is no
supported release yet, so fixes land on `main`.
