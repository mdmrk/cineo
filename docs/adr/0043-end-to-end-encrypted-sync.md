# 0043. End-to-end encrypted sync through our own server

- Status: Accepted
- Date: 2026-11-27
- Supersedes: the "no cloud sync for now" exclusion in 0010 and
  `docs/COMPATIBILITY.md` (owner decision 2026-10-06). Amends 0007
  ("Sync, if ever, is our own design": this is that design).

## Context
The owner asked on 2026-11-27 for sync between a user's devices. The
owner has already chosen the direction: our own minimal server, free
software in this repo, end-to-end encrypted, no accounts. A device makes a
sync key, shown as a QR code and a text code; another device scans or pastes
it. Sync is off by default, and the server URL can be changed in Settings.
Hosting target: a Cloudflare Worker with D1.

### What the app stores today (VERIFIED: source, 2026-11-27)
- The core asks for five persistence effects
  (`crates/cineo-core/src/app/state.rs`): `SaveAddons(Vec<TransportUrl>)`,
  `SaveLibraryItem(LibraryItem)`, `DeleteLibraryItem(String)`, `ClearLibrary`
  and `SaveSettings(Settings)`. `cineo_store::Store::apply` runs them.
- Schema (`cineo-store/src/migrate.rs`, version 4): `addons(position,
  transport_url)`, `library_items(id, content_type, name, poster, video_id,
  time_offset_ms, duration_ms, updated_ms, stream, favorite)` and
  `settings(key, value)`.
- `LibraryItem.updated_ms` is "last watched", not a modification time:
  `remove_from_library` sets it to 0 for a favourite it keeps. It cannot
  order edits. `favorited` is an ordering counter (`max + 1`).
- `SavedStream` holds the addon transport URL. Transport URLs often embed
  API keys (`docs/SECURITY.md` §Assets).
- While playing, the player reports progress every 5 s
  (`cineo-player/src/tracker.rs`, `PROGRESS_INTERVAL`). Each report saves
  the library item.
- `Settings` has 41 keys (`app/settings.rs`). They include security choices
  (`allow_private_network`), the P2P disclosure (`p2p_acknowledged`) and
  hardware-specific ones.
- `AddonClient` in `cineo-net` only sends GET requests.

### Cloudflare free plan (VERIFIED: official docs, fetched 2026-11-27)
| Limit | Free | Paid ($5/month minimum) | Source page |
|---|---|---|---|
| Worker requests | 100,000/day, reset 00:00 UTC; above it, error 1027 | 10 M/month included, then $0.30/M | workers/platform/limits, /pricing (modified 2026-11-25, 2026-11-18) |
| Worker CPU per request | 10 ms | 30 s default, 5 min max | same |
| D1 rows written | **100,000/day** (confirmed) | 50 M/month, then $1.00/M | d1/platform/pricing (modified 2026-10-30) |
| D1 rows read | 5 M/day | 25 G/month | same |
| D1 storage | 5 GB in total, **500 MB per database**, 10 databases | 10 GB per database | d1/platform/limits (modified 2026-11-20) |
| D1 row / BLOB size | 2,000,000 bytes | same | same |
| D1 bound parameters per query | 100 | same | same |
| D1 queries per invocation | 50 (a `batch()` counts as one) | 1,000 | same |
| KV writes | 1,000/day; 1 write/s per key | — | kv/platform/limits (modified 2026-10-30) |

- In D1, INSERT, UPDATE and DELETE each count every row they touch, and a
  write to an indexed column also writes one index row. Row size does not
  change the count (VERIFIED: D1 pricing page). Whether an UPSERT counts as
  one row or two is not stated (UNKNOWN; measured in step 5).
- When the free daily limit is reached, D1 queries fail until 00:00 UTC
  (VERIFIED: D1 pricing page).
- The Workers Rate Limiting binding allows periods of 10 or 60 s. Its counts
  are approximate and kept per Cloudflare location (VERIFIED: its docs page,
  modified 2026-11-12). Whether it is available on the free plan is
  UNKNOWN. The page does not say.
- Whether Cron Triggers, which we would use to delete idle spaces, are
  available on the free plan was not checked (UNKNOWN).

### F-Droid (VERIFIED: f-droid.org/docs/Anti-Features, fetched 2026-11-27)
- `TetheredNet` is not applied when "a simple configuration option" points
  the app "to a running instance of an alternative, publicly available,
  self-hostable server solution".
- `NonFreeNet` covers apps that "promote or depend entirely on a
  proprietary network service". The page says nothing about opt-in.
- Our recipe already declares `NonFreeNet` because of Cinemeta
  (`docs/fdroid/io.github.mdmrk.cineo.yml`). Sync neither adds nor removes
  that flag (INFERRED).
- Whether a Worker plus D1 counts as "self-hostable" depends on running it
  outside Cloudflare. `workerd` is open source and wrangler emulates D1
  locally with SQLite (INFERRED from memory, UNKNOWN until step 5 tries it
  and documents it).

### Crates (VERIFIED: crates.io API, 2026-11-27)
| Crate | Version | License | Updated | In `Cargo.lock` now |
|---|---|---|---|---|
| `aws-lc-rs` | 1.19.0 | ISC AND (Apache-2.0 OR ISC) | 2026-10-28 | yes; reaches `cineo-net` through reqwest → rustls, also on Android |
| `data-encoding` | 2.11.1 | MIT | 2026-08-03 | yes (transitive) |
| `zeroize` | 1.9.2 | Apache-2.0 OR MIT | 2026-11-17 | yes (transitive) |
| `qrcode` | 0.15.0 | MIT OR Apache-2.0 | 2026-11-19 | no |
| `chacha20poly1305` | 0.11.0 | Apache-2.0 OR MIT | 2026-08-05 | no |
| `hkdf` | 0.13.0 | MIT OR Apache-2.0 | 2026-03-30 | no |
| `argon2` | 0.6.0 | MIT OR Apache-2.0 | 2026-08-27 | no |
| `bip39` | 3.0.0 | CC0-1.0 | 2026-09-17 | no |
| `worker` (workers-rs) | 0.9.0 | Apache-2.0 | 2026-11-11 | no |

`aws-lc-rs` 1.19.0 exports `hkdf::HKDF_SHA256`, `hmac`,
`aead::CHACHA20_POLY1305`, `aead::AES_256_GCM_SIV` and `rand`
(VERIFIED: source in the cargo registry).

Android QR scanning (VERIFIED: Maven Central, 2026-11-27):
`io.github.zxing-cpp:android` 2.3.0 (2025-02-19, Apache-2.0),
`com.google.zxing:core` 3.5.3 (2024-01-31). Google ML Kit pulls in Play
Services, which F-Droid flags or rejects (INFERRED: `NonFreeDep`).
Android has no `cineo://` intent filter today (VERIFIED:
`AndroidManifest.xml`).

## Options considered
1. **Whole-state snapshot.** One encrypted blob per space, replaced on each
   push with a version check. Simple, but each progress push re-uploads the
   whole library, and two devices conflict on every concurrent change.
2. **One encrypted record per item, last-writer-wins by hybrid logical
   clock, with a per-record version check on the server.** Small pushes;
   merge is pure and testable; the server stays a dumb store.
3. **Operation log (CRDT-style event stream).** Most flexible, but the log
   grows without bound and needs compaction. Too much for what we sync.

Server language:
- **a. TypeScript Worker.** No runtime dependencies, the first-class D1
  binding, tests with Cloudflare's Vitest pool.
- **b. workers-rs (`worker` 0.9.0).** Same language as the app, but a Wasm
  bundle under a 10 ms CPU limit, and the crate is still 0.x.

## Decision
Option 2, with server option a.

### 1. What is synced
| Record name | Content | Source effect |
|---|---|---|
| `library/<meta id>` | every `LibraryItem` field, or a tombstone | `SaveLibraryItem`, `DeleteLibraryItem` |
| `library-cleared` | only a stamp: items stamped earlier are hidden | `ClearLibrary` |
| `addons` | the ordered transport-URL list, as one value | `SaveAddons` |
| `setting/<key>` | the encoded value (`Setting::value`) | `SaveSettings`, synced keys only |

Settings **not synced** (device-local):

| Key | Why it stays on the device |
|---|---|
| `allow_private_network` | Security: a stolen sync key must never open the LAN to addons |
| `p2p_acknowledged`, `p2p_enabled` | The P2P disclosure is shown and accepted on each device (LEGAL.md); P2P on mobile data is a per-device choice |
| `torrent_upload`, `torrent_storage`, `download_limit`, `upload_limit`, `peer_limit`, `torrent_dht`, `torrent_port` | Disk, memory, network and ports differ per device |
| `hardware_decoding`, `audio_output`, `audio_passthrough` | Hardware |
| `volume`, `remember_volume` | Speakers and headphones differ |
| `interface_scale`, `start_page`, `on_leave`, `pause_on_minimize`, `escape_leaves_fullscreen` | Screen and form factor (desktop vs phone) |
| `subtitle_size`, `subtitle_position` | Depend on screen size and distance |
| `check_for_updates` | Per installation |
| the sync settings themselves | Key, server URL, device id |

Everything else is synced: languages, subtitle font, colour, outline,
background and opacity, `keep_subtitle_styles`, seek steps,
`hide_controls`, `watched_at`, `binge_watching`, `next_video_notice` and
`ui_language`. The split is one list in the core,
`settings::DEVICE_LOCAL`. A new setting is synced by default unless it is
added to that list, and a test enumerates `Settings::all()` so that a new
key needs a decision.

### 2. Merge model (`cineo_core::sync`, pure)
- **Stamp** = `(physical_ms, counter, device_id)`, a hybrid logical clock
  (Kulkarni et al., 2014). It is totally ordered and compared field by field.
  `device_id` is a random `u64` per installation that is never synced.
- **Local edit:** `tick(now_ms)` returns `max(now_ms, last.physical)`, and
  bumps the counter when the physical part did not grow. The clock never
  goes backwards, even if the wall clock does.
- **Remote record seen:** `observe(remote, now_ms)` moves the local clock
  past the remote stamp. An edit made after a device has seen a change
  therefore always wins over it, even when that device's wall clock is
  hours behind. Only truly concurrent edits are decided by wall clock, then
  counter, then device id. With skewed clocks the device whose clock runs
  fast wins those. This is inherent to last-writer-wins and is accepted.
- **Per record:** keep the value with the larger stamp. A tombstone is a
  value. An item is visible when it is not a tombstone and its stamp is
  greater than `library-cleared`'s stamp.
- **The merge is a join** (max by stamp), so it is commutative, associative
  and idempotent. Any order of pulls converges.
- **Joining** a space with local data: local records keep their stamps and
  are pushed. The `addons` value is the union, with the space's order
  first, so installing addons on two devices before pairing loses none.
- **Unknown record kinds or format versions** are kept as they are and
  ignored, so an older app does not destroy a newer one's records.
- Time comes in with the action (`now_ms`), as `PlaybackProgress` already
  does. The core decides when to push and emits effects: `SyncPull { since }`,
  `SyncPush(Vec<SyncRecord>)` (plaintext) and `SaveSyncState`. Results come
  back as `SyncPulled` and `SyncPushed` actions.
- **Batching:** the record is marked dirty on every local edit. A push
  happens 60 s after the last change, at most once every 5 min while
  playing, and immediately on playback stop, pause or exit (best effort).
  A pull happens on start, when Android comes to the foreground, and every
  10 min while the app is open.

Core tests (step 2):
`stamps_order_by_time_then_counter_then_device`,
`the_clock_never_goes_back_when_the_wall_clock_does`,
`an_edit_after_seeing_a_remote_change_wins_with_a_slow_clock`,
`a_newer_remote_item_replaces_the_local_one`,
`an_older_remote_item_is_ignored_and_the_local_one_stays_dirty`,
`a_tombstone_beats_an_older_edit_and_loses_to_a_newer_one`,
`clearing_hides_older_items_but_not_newer_ones`,
`merging_in_any_order_gives_the_same_state` (every permutation of a fixed
set of records from three devices, including skewed clocks and repeats),
`two_devices_converge_after_exchanging_changes`,
`device_local_settings_are_never_pushed_or_applied`,
`every_setting_is_either_synced_or_device_local`,
`joining_keeps_the_addons_of_both_devices`,
`unknown_record_kinds_are_kept_and_ignored`,
`progress_pushes_at_most_every_five_minutes_while_playing`,
`stopping_playback_pushes_at_once`.

### 3. Key, encryption and revocation (`cineo_net::sync`)
- **Sync key:** 16 random bytes (128 bits) from `aws_lc_rs::rand`.
- **Text code:** RFC 4648 base32 of `key ‖ check`, where `check` is the
  first 2 bytes of SHA-256(`"cineo-sync-check" ‖ key`). That is 29
  characters, in groups of 4 separated by `-`. Input is read
  case-insensitively and ignores spaces and dashes. The checksum catches
  typos before any request is sent.
- **QR code:** the link `cineo://sync/<code>`, plus `?server=<url>` when the
  server is not the default. Encoded with `qrcode` (rendering in each
  shell). On Android, the system camera can open the link, which needs a
  new intent filter for `cineo://sync`. An in-app scanner (zxing-cpp,
  Apache-2.0) is left for later (Consequences).
- **Derivation:** HKDF-SHA256 with the key as IKM and salt
  `"cineo-sync-v1"`, three outputs of 32 bytes:
  - `info = "token"` → **bearer token**. The server addresses the space by
    `SHA-256(token)` and stores nothing else about the client. Knowing the
    token is the only authorization.
  - `info = "names"` → HMAC key. The server-side record key is
    HMAC-SHA256(names, record name), truncated to 16 bytes, so the server
    never sees meta ids or setting names.
  - `info = "seal"` → AEAD key.
- **Encryption:** ChaCha20-Poly1305 (`aws_lc_rs::aead`). Blob =
  `format version (1 byte) ‖ random 96-bit nonce ‖ ciphertext ‖ tag`, with
  AAD = `format version ‖ server record key`. The server cannot move a blob
  to another record or downgrade the format. The plaintext is JSON with the
  record name, stamp and value. Random 96-bit nonces are safe far beyond our
  volume, under 2³² messages per key (INFERRED: standard birthday bound).
  Key material is wrapped in `zeroize`.
- **Why no password hashing (`argon2`):** the key is random, not chosen by
  a person. A slow KDF adds nothing at 128 bits of entropy.
- **No new crypto dependency:** `aws-lc-rs` is already compiled into
  `cineo-net` on every target (VERIFIED: `cargo tree`). `data-encoding` and
  `zeroize` are already in the lock file, and `qrcode` is the only new
  crate.
- **Show the code again:** any paired device can show the QR and code (the
  key is stored locally).
- **Rotate** ("Change sync key", for a lost or stolen device): a new key is
  made, all current records are pushed to the new space, the old space is
  deleted with `DELETE /v1/space`, and the new code is shown. Other devices
  get 404 on the old space and show "Sync key changed: pair again". The
  holder of the old key keeps whatever they downloaded before the rotation.
  Nothing can take that back.
- **Leave** on one device: forgets the key, keeps local data. Optionally
  "Also delete the data on the server".
- **Lost key:** a remaining paired device can show it again. If every
  device is lost, the encrypted data cannot be recovered by anyone,
  including the server operator. That is by design. The space expires after
  the inactivity period (§4).

### 4. Protocol (`/v1`, JSON, HTTPS only)
All requests go through `cineo-net`'s `AddonClient` and `NetPolicy`
(ADR-0006). The client gains POST and DELETE with a body cap. The sync
server is treated like an addon host: a private-network address needs the
existing opt-in. `Authorization: Bearer <base64url token>` is sent on every
request except `/v1/info`. The token, code and key never appear in logs or
errors.

| Endpoint | Purpose |
|---|---|
| `GET /v1/info` | `{ protocol: 1, max_record_bytes, max_batch_bytes, max_batch_records, max_space_records, max_space_bytes }` |
| `GET /v1/records?since=<seq>&limit=<n≤500>` | Records with `seq > since`, by `seq`: `{ records: [{ key, seq, blob }], more }`. 404 if the space does not exist |
| `POST /v1/records` | `{ writes: [{ key, base_seq, blob }] }`. Applied **atomically** (one D1 `batch()`). Each `base_seq` must equal the record's current `seq` (0 = new), or the whole batch fails with **409** and the conflicting keys. The space is created on its first write. Response: `{ seqs: [...] }` |
| `DELETE /v1/space` | Deletes the space and its records |

- **Concurrency control:** a per-space counter `seq`, raised on every
  write, acts as each record's version (compare-and-set per record). On
  409, the client pulls, merges in the core and retries; the merge
  decides, not the server. Per-record versions mean that two devices
  editing different items never conflict. A whole-space ETag would make
  every concurrent push a conflict.
- **Versioning:** path prefix `/v1`, `protocol` in `/v1/info`, and a format
  byte in each blob. A breaking change adds `/v2` and the server keeps `/v1`
  until no supported release uses it.
- **Size limits** (server-enforced, Content-Length checked before reading,
  body read with a cap): blob ≤ 32 KiB, request ≤ 256 KiB and ≤ 100 writes,
  space ≤ 10,000 records and ≤ 8 MiB. The client's response cap for sync is
  2 MiB (one pull page).
- **Abuse limits:** the token is required; a space exists only after a
  valid write; quotas per space as above; requests per token hash and new
  spaces per IP limited with the Rate Limiting binding if the free plan
  has it (UNKNOWN), else with a Cloudflare WAF rate-limiting rule; spaces
  without a write for 365 days are deleted by a scheduled job. When D1 or
  the Worker hits its daily limit, the server returns 503 with
  `Retry-After` until 00:00 UTC, and the client backs off and keeps working
  locally.
- **D1 schema:** `spaces(id BLOB PRIMARY KEY, seq INTEGER, records INTEGER,
  bytes INTEGER, touched_ms INTEGER) WITHOUT ROWID` and `records(space BLOB,
  key BLOB, seq INTEGER, blob BLOB, PRIMARY KEY (space, key)) WITHOUT
  ROWID`, plus an index on `records(space, seq)`.

### 5. Where each part lives
- **Merge, stamps, record model, settings split, push and pull planning:**
  module `cineo_core::sync`. No IO, no clock, no randomness. The device id
  and `now_ms` are inputs.
- **Sync state** (key, server URL, device id, clock, per-record stamp,
  server `seq`, dirty flag): new tables in `cineo-store`, one migration plus
  a fixture. The key sits in `cineo.db` (mode 0600, as now).
- **Client** (code, derivation, AEAD, HTTP): module `cineo_net::sync`. No
  new crate. It is the same IO boundary (HTTP through `NetPolicy`), and the
  crypto library is already in that crate's graph (ADR-0002: a crate is a new
  dependency or IO boundary, and this is neither).
- **Server:** `server/sync/`, a TypeScript Worker with no runtime
  dependencies (dev only: wrangler, TypeScript, Vitest with
  `@cloudflare/vitest-pool-workers`, versions researched in step 5). It is
  MIT like the rest of the repo, with its own CI job and a self-hosting
  guide.
- **QR rendering:** `qrcode` in `cineo-desktop` and `cineo-android` (the
  latter hands Kotlin the module matrix).

### 6. Cost per active user (INFERRED estimates, to be measured in step 5)
Assumed: 2 h of playback a day, the app open 3 h, a library of 600 items
(~1 KB per sealed record).
- **Pushes:** about 30 a day (every 5 min while playing, plus stops and
  edits), about 1.2 records each. Rows written per push is about
  2 × records (row + `seq` index) + 1 (space row), so **≈ 100 rows
  written/day**.
- **Pulls:** about 25 a day at ≈ 3 rows read each, so **≈ 75 rows
  read/day**.
- **Requests:** **≈ 55/day**.
- **Storage:** ≈ 1 MB per user, records and index.
- **Without batching** (one push per 5 s progress save) the same day would
  be ≈ 1,440 pushes and ≈ 4,300 rows written. Batching is required.

| Limit (free) | Users it allows |
|---|---|
| 100,000 requests/day | ~1,800 daily active |
| 100,000 rows written/day | ~1,000 daily active |
| 5 M rows read/day | far beyond the others |
| 500 MB per database | **~500 users in total** (the first limit hit; sharding across the 10 free databases would give ~5,000) |
| 10 ms CPU per request | a 256 KiB batch should fit (UNKNOWN until measured) |

On the $5 plan, the same model allows about 6,000 daily active users by
requests, about 16,000 by rows written and about 10,000 users per 10 GB
database.

### 7. Security and privacy
- Off by default. Nothing is sent until the user turns sync on or pastes a
  code.
- **The server sees:** the client IP, request times and sizes, the token
  hash, the number of records, and opaque 16-byte keys. **It does not
  see:** titles, ids, progress, addon URLs or settings.
- A malicious or compromised server can withhold records, delete the space
  or replay an older blob of the same record. It cannot read, forge or move
  records. Stale replays lose to newer local stamps but can mislead a newly
  paired device (accepted; Consequences).
- **Whoever holds the code reads everything synced**, including addon
  transport URLs, which can carry API keys. The UI says so where the code
  is shown, and "Change sync key" is one action away.
- Decrypted records are still parsed leniently with strict domain types
  (ADR-0003), and `SavedStream::parse` keeps rejecting unplayable sources.
  Other versions or a buggy device may write bad data.
- Synced data can never change device-local security settings
  (`device_local_settings_are_never_pushed_or_applied`).

### 8. Delivery (one PR each; each says how it was verified)
1. This ADR and the doc sync below. No code.
2. `cineo_core::sync`: stamps, merge, settings split, push and pull
   planning, with the tests in §2.
3. `cineo-store`: sync tables, migration and fixture.
4. `cineo-net::sync`: code, derivation, AEAD with known-answer vectors,
   endpoints, tested against a wiremock server (409 → pull, merge, retry;
   size caps; a tampered or moved blob is rejected; a typo in the code is
   rejected).
5. `server/sync/`: Worker, D1 schema, limits and tests; CI job;
   measurements of rows written, UPSERT counting and CPU per request;
   self-hosting guide tried outside Cloudflare; deploy of the default
   instance.
6. Desktop: Settings → Sync (turn on, QR and code, paste a code, server
   URL, change key, leave, last sync status). `cineo://sync/` deep link.
7. Android: the same screen, the intent filter for `cineo://sync`, and
   pull on foreground.
8. Raise "Account / cloud sync" in `docs/COMPATIBILITY.md` only with named
   tests.

## Consequences
- Cineo gains a network service that we run. Its availability, quotas and
  abuse handling are ours to maintain. The app keeps working fully offline
  and without sync.
- A non-Rust toolchain enters the repo (Node, for `server/sync/`).
  `cargo-deny` does not cover it, so its lockfile and `npm audit` get their
  own CI step.
- Known limitations to record in `docs/TECHNICAL_DEBT.md` when they ship:
  - Item-level last-writer-wins: favouriting on one device while the other
    saves progress for the same item, both before syncing, loses one of
    them.
  - Concurrent `favorited` counters can tie, so favourites order by id
    after them.
  - The addon list is one value: concurrent reorders lose one, except when
    joining.
  - Tombstones are never collected.
  - A device with a clock years ahead pushes everyone's clock forward
    (harmless for order, odd in diagnostics).
  - The sync key is stored in `cineo.db`, not in the OS keychain.
  - A malicious server can replay old records to a newly paired device.
  - No in-app QR scanner on Android: the system camera hands the link
    over, and the code may stay in the camera app's history (UNKNOWN per
    camera app).
- Docs in the same change as this ADR:
  - `docs/COMPATIBILITY.md`: remove "Cloud or account sync" from the
    exclusions; the "Account / cloud sync" row stays Unsupported with
    "Not built yet (ADR-0043)".
  - `docs/ARCHITECTURE.md`: a sync boundary section, marked as not built.
    The overview and the crate table change when the code lands.
  - `AGENTS.md`: replace "no cloud sync for now" with "sync only through
    our own end-to-end encrypted server (ADR-0043)", and note the
    `server/` exception to "all HTTP goes through `cineo-net`" (it is the
    server, not a client).
  - `docs/adr/README.md`: index row; 0010 and 0007 marked amended by 0043.
- Docs with the later steps: `docs/SECURITY.md` (§Sync, step 4),
  `docs/DEVELOPMENT.md#dependencies` (`qrcode`, the server's dev
  dependencies), `docs/ADDON_PROTOCOL.md` (no change: sync is not addon
  protocol).

## Rejected alternatives
- **Whole-state snapshot (1):** re-uploads the library on every progress
  push, and every concurrent change conflicts.
- **Operation log (3):** unbounded growth and compaction for data that is
  naturally "latest value per item".
- **Cloudflare KV:** 1,000 writes a day on the free plan and 1 write/s per
  key (VERIFIED). Too few even for one heavy user.
- **workers-rs (b):** the server does no crypto and shares no domain
  types with the app (it stores opaque blobs), so Rust buys nothing there.
  A Wasm bundle under a 10 ms CPU limit and a 0.x SDK are risks without
  a benefit (INFERRED).
- **Google Drive appData:** OAuth app verification, Google users only, an
  F-Droid anti-feature (owner decision).
- **Exported file or folder:** works today by copying `cineo.db`
  (COMPATIBILITY.md), but is manual and has no merge (owner decision).
- **Stremio accounts:** private API (ADR-0007).
- **Email and password accounts:** a password database to protect, password
  reset, and personal data on our server. The owner chose no accounts.
- **Passphrase chosen by the user, hashed with `argon2`:** weak passphrases
  would make a stolen database of spaces brute-forceable. A random key
  avoids that.
- **AES-256-GCM-SIV:** also in `aws-lc-rs` and nonce-misuse resistant, but
  ChaCha20-Poly1305 is fast without AES instructions on older arm64 phones,
  and random 96-bit nonces stay within bounds at our volume (INFERRED).
- **In-app QR scanner with ML Kit:** Play Services, not acceptable on
  F-Droid. zxing-cpp is possible later, at the cost of the `CAMERA`
  permission and a new Android dependency.
