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
| Tampered downloads                                                                       | `install.sh` and `install.ps1` check every file against the release's `SHA256SUMS` before installing anything, and stop if one has no line in it; release files also have build-provenance attestations. `SHA256SUMS` is signed with minisign by a key only the `release-signing` environment can read (`minisign.pub`); `install.sh` refuses a latest release whose signature is missing or does not verify when `minisign` is installed, and skips the check without it. `install.ps1` does not check the signature. Checksums alone protect against corrupted or swapped mirror files; the signature also against release files replaced by someone who can write to the repository but not read the key.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Tampered in-app updates (Windows and Linux, ADR-0040, ADR-0041) | Nothing is replaced until `SHA256SUMS.minisig` verifies with the public key built into Cineo (`self_update::PUBLIC_KEY`, kept equal to `minisign.pub` by `scripts/release.sh key`), its trusted comment names exactly the offered version (no replay of an older signed release), and the executable and license zip match their lines. Downloads go through `cineo-net` with public HTTPS only, at most 256 MiB and 10 minutes. Only an update the user clicks is downloaded. A leaked signing key would let someone ship an update; it lives only in the `release-signing` environment and there is no rotation path yet. |

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
- Android keeps the library, addon URLs and sync key out of Android backups:
  `allowBackup="false"`, plus backup rules that exclude every domain from
  cloud backup and from device-to-device transfer, which `allowBackup`
  alone does not stop on Android 12 and later (`AndroidManifest.xml`,
  `res/xml/`; checked by Android Lint, not by a test).

### Sync (ADR-0043)

The client and both server hosts exist, the Cloudflare Worker is
deployed, and the desktop and Android turn sync on in Settings → Data.
Sync is off by default.

- **Key.** 16 random bytes from `aws_lc_rs::rand`, stored in the private
  database (`sync_pairing`). It is shown as a 32-character code with a
  4-byte checksum, so a typo is caught before any request
  (`a_mistyped_code_is_rejected`). Its `Debug` output hides it
  (`the_sync_key_is_never_printed`, `the_keys_are_never_printed`).
- **Derived keys.** HKDF-SHA256 gives an Ed25519 signing key, an HMAC key
  for the record names and a ChaCha20-Poly1305 key. The derivation is
  pinned by vectors computed separately with Python's `hmac` and `hashlib`
  and, for the public key, `openssl`
  (`derived_keys_match_the_reference_vectors`).
- **Signed requests.** Every request carries an Ed25519 signature over the
  method, path, query, time and body hash, checked by the client's tests
  with an independent verifier
  (`pull_opens_records_and_skips_unreadable_ones`,
  `push_sends_sealed_writes_with_their_base_versions`). The server refuses
  a missing or wrong signature, or one for another query, body, method or
  key (`records_need_a_valid_signature`), and a time more than 10 minutes
  from its clock (`a_signed_request_expires_after_ten_minutes`). A captured
  request can therefore be replayed only within those minutes, and the
  server's database holds nothing that lets anyone sign.
- **What the server receives.** The public key, 16-byte record keys and
  sealed blobs. Ids, titles, addon URLs and settings never leave the device in
  clear (`push_sends_sealed_writes_with_their_base_versions`).
- **Integrity.** A blob opens only with its key, under its own record key
  and in a known format; a tampered, moved or foreign blob is skipped
  (`a_sealed_record_opens_only_with_its_key_in_its_place`,
  `pull_opens_records_and_skips_unreadable_ones`). Opened records are
  parsed again leniently (ADR-0003). A server can still withhold, delete
  or replay old records.
- **Transport.** The sync server goes through `AddonClient` and
  `NetPolicy`, so private addresses need the opt-in
  (`the_sync_server_follows_the_network_policy`). It must be `https`, or
  `http` to this machine only
  (`the_server_must_use_https_unless_it_is_this_machine`). reqwest 0.13.5
  drops the `Authorization` header when a redirect changes host, port or
  scheme (VERIFIED: its `remove_sensitive_headers`). Pages are capped at
  2 MiB (`an_oversized_pull_page_is_refused`) and the push answer at 64 KiB.
- **Device-local settings** such as private-network access are never
  pushed or applied from another device
  (`device_local_settings_are_never_pushed_or_applied`).
- **Who holds the code reads everything synced**, including addon URLs
  that can carry API keys. The desktop says so next to the code and its
  QR code. A `cineo:///sync/<code>` link only fills the code; pairing
  waits for a click on "Pair", and a typed server must be `https`
  (`a_sync_link_fills_the_code_and_opens_data_settings`,
  `turning_on_needs_an_https_server`). After a lost device, "Change sync
  key" makes a new key and deletes the data under the old one
  (`a_new_key_moves_everything_then_deletes_the_old_space`). On Android the
  same holds (`a_bad_code_or_an_insecure_server_turns_nothing_on`,
  `a_sync_link_gives_its_code_and_server`), and the copied code is marked
  sensitive on Android 13 and later (`EXTRA_IS_SENSITIVE`, no test), so the
  system does not preview it.
- **Camera (Android).** The app declares the `CAMERA` permission only for
  the sync QR scanner, asks for it the first time "Scan QR code" is tapped,
  and uses the camera only while the scanner is open; frames are read in
  memory and not stored. Without the permission, or without a camera
  (`android.hardware.camera.any` is optional), the code is typed.
- **Local network (Android 17).** The app declares
  `ACCESS_LOCAL_NETWORK` and asks for it only when "Allow local network
  addresses" is turned on, or from that row's "Allow" button. Without it,
  Android blocks LAN addresses even with the setting on (no automated test).
- **Server (`cineo-sync-server`).** It stores SHA-256 of each space's
  public key and only sealed blobs under opaque keys
  (`the_server_stores_nothing_readable`). A request without a valid
  signature is refused (`records_need_a_valid_signature`). Creating a
  space needs a 20-bit proof of work bound to its key; later pushes do not
  (`only_a_new_space_needs_proof_of_work`), and the client does the work
  when asked but refuses more than 24 bits
  (`a_new_space_is_pushed_again_with_the_work_the_server_asks_for`,
  `too_much_work_is_refused_without_trying`). Malformed
  pushes, more than 64 writes, wrong key sizes, empty or oversized blobs
  and repeated keys are refused (`malformed_pushes_are_refused`); a body
  over 256 KiB is refused before it is read
  (`an_oversized_request_is_refused_before_it_is_read`). A space holds at
  most 10,000 records and 8 MiB
  (`a_full_space_refuses_more_but_takes_replacements`). Each address may
  make 120 requests a minute and 10 new spaces an hour
  (`requests_are_limited_per_address_and_minute`,
  `new_spaces_are_limited_per_address_and_hour`,
  `too_many_requests_from_one_address_are_refused`); behind a proxy, only
  when started with `--trust-forwarded-for`
  (`behind_a_proxy_the_forwarded_address_is_limited`). A deleted space
  stays deleted (`a_deleted_space_is_gone_and_cannot_come_back`), and
  spaces without a write for 365 days are swept
  (`idle_spaces_and_deletion_markers_are_swept`). The binary serves plain
  HTTP and is meant to sit behind a TLS reverse proxy
  ([SYNC_SERVER.md](SYNC_SERVER.md)). The Cloudflare Worker host has the
  same checks; its rate limits are Cloudflare's approximate, per-location
  Rate Limiting bindings, 120 requests a minute per space and 10 new
  spaces a minute per address, which let short bursts through (ADR-0043).
  New spaces ask the limiter only when a push would create one
  (`only_new_spaces_ask_for_permission`).

### Remote control (ADR-0044)

- **On by default** (owner's decision, 2026-11-27). The desktop's first
  start makes a key and listens on TCP 18765 on every IPv4 interface, or a
  port the system picks. "Turn off" in Settings → Remote control closes
  the listener and its open connections, forgets the key, and is
  remembered across restarts (`stopping_the_listener_closes_the_port`,
  `the_remote_control_is_on_unless_it_was_turned_off`,
  `remote_pairings_survive_a_reopen_and_turning_off_is_remembered`).
- **Key.** 16 random bytes, shown as a 32-character code and a QR code.
  Whoever has the code and reaches the port can control playback and see
  what plays; nothing else is exposed. "New code" replaces it. The key is
  stored in `remote_host` and the phone's copy in `remote_peer`; sync never
  reads either table.
- **Sealing.** Each request and answer is sealed with ChaCha20-Poly1305
  under a key derived from the code with HKDF-SHA256; the answer is bound
  to its request's nonce. A request with another key gets 401 and runs
  nothing (`a_phone_with_another_key_is_refused_and_nothing_runs`), and an
  answer cannot be moved to another request
  (`a_captured_request_cannot_be_replayed`).
- **Replays.** A request more than 2 minutes from the desktop's clock, or
  with a nonce already seen in that window, gets 403
  (`a_request_far_from_the_desktops_clock_is_refused`,
  `a_captured_request_cannot_be_replayed`,
  `a_request_is_admitted_once_and_only_near_the_desktops_time`). At most
  4,096 nonces are remembered; past that, 503
  (`old_nonces_are_forgotten_and_the_memory_is_bounded`). Commands are
  absolute, so one that ran twice has the same effect.
- **Playing a sent stream.** `Command::Play` carries a `SavedStream`
  parsed with the library's rules: playable sources only, http(s) images,
  ids of at most 512 characters, names cut to 200 characters. The desktop
  then applies its own checks as for its own detail page (scheme, the P2P
  switch, the P2P notice). Whoever holds the key can make the computer
  play any stream an addon could have offered.
- **Limits.** One route (`POST /v1/remote`), bodies of at most 32 KiB, 16
  connections, headers within 10 s, a connection within 15 s, no
  keep-alive (`the_desktop_answers_nothing_else`).
- **Commands.** A closed set with checked ranges
  (`commands_out_of_range_or_unknown_are_refused`), turned into typed
  player commands on the UI thread; nothing from the phone reaches mpv as
  text. The answer's title and track names are cut to 200 and 60
  characters without control characters
  (`a_status_is_read_leniently_and_kept_small`).
- **Phone side.** The phone only pairs with private, loopback or
  link-local addresses, so a link cannot send it to the internet
  (`only_local_network_addresses_can_be_paired`,
  `the_phone_only_talks_to_the_local_network`). Its requests go through
  `cineo-net` with private addresses allowed, no redirects and a 5 s
  timeout. On Android 17 the remote screen asks for `ACCESS_LOCAL_NETWORK`
  (no automated test).
- **Through sync.** The desktop's address and remote key travel in a
  `computer/<device id>` sync record, sealed like every sync record
  (ADR-0043): the sync server never sees them, and whoever holds the sync
  code can also control the computer. The phone checks that a synced
  address is on the local network, as for a typed one
  (`a_computer_record_survives_its_json_and_bad_ones_are_unreadable`).
- **Not protected.** Anyone on the network sees that the port is open and
  the size and timing of requests. A firewall may block the port; Cineo
  does not change firewall rules.

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
- While a video plays or its torrent connects, the window title names it, so window lists, task
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
  embedded gzip-compressed and written to `<cache dir>\libmpv\<CRC32>\`,
  named by the checksum in the gzip trailer, and replaced when its size
  differs; a damaged archive fails its CRC check and writes nothing. A `libmpv-2.dll` next to the executable is loaded first, so
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
  `actions/*` by an exact version tag (`@vX.Y.Z`) and every other action
  by a full commit hash, listed with its version in `docs/DEVELOPMENT.md`
  (a moved tag of a third-party action cannot reach the signing key or the
  attestations);
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
| Sync                              | Off; turned on in Settings → Data (ADR-0043)                                                                                                        |
| Remote control                    | On (desktop): TCP 18765 on the LAN from the first start, sealed with a random key; off in Settings → Remote control (ADR-0044)                       |
| mpv                               | libmpv in-window; no user config, scripts or ytdl; argument-array `loadfile` only; http(s) only; forward cache at most 120 s and 512 MiB (ADR-0024) |

## Unsafe code

`unsafe_code = "deny"` workspace-wide, and every crate root without FFI
(`cineo-core`, `cineo-net`, `cineo-store`, `cineo-stream` and both roots of
`cineo-desktop`) adds `#![forbid(unsafe_code)]`, so no module in them can
lift it. A crate that genuinely needs FFI must:

1. Keep no `forbid` in its root.
2. Justify it in an ADR.
3. Keep `unsafe` inside the modules that lift the lint, with a `reason`.

The exceptions are `cineo-player` (ADR-0014) and `cineo-android`
(ADR-0028), which use the workspace lints like every crate. Only these
modules allow it: `embedded::ffi` (libmpv bindings) and
`embedded::render` (the OpenGL renderer) in `cineo-player`, and `native`
(the JNI entry point for the TLS verifier) in `cineo-android`.

## Reporting a vulnerability

Do not open public issues for vulnerabilities. Use GitHub's private
vulnerability reporting on this repository ("Security" tab → "Report a
vulnerability"). We aim to acknowledge reports within 7 days. Only the
latest release is supported; fixes land on `main` and ship in the next
release.
