# 0012. Torrent engine: librqbit, with BitTorrent traffic outside `cineo-net`

- Status: Accepted (the crates.io `librqbit` as is, and its storage
  layout's use as a cache: superseded by 0015)
- Date: 2026-10-06

## Context
ADR-0010 decided on our own local streaming engine (`cineo-stream`) and left
the library choice open until a spike. The owner moved M9 (torrents) ahead of
M6/M7 on 2026-10-06.

What popular addons send (VERIFIED, 2026-10-06): Torrentio's stream response
for `tt0063350` had 28 streams. Every one had `infoHash`, `fileIdx`,
`behaviorHints.filename` and `bingeGroup`, and none had `url` or `sources`.
So the engine must find peers from a bare info hash (DHT).

Spike (VERIFIED, 2026-10-06, Linux, `librqbit` 9.0.1 with `rust-tls`,
throwaway code outside the repository): one file of a public-domain film's
torrent (archive.org, `Night of the Living Dead`, 568 MiB MP4) was served
over a loopback HTTP server with range support and played in headless
mpv 0.41.
- From a `.torrent` file: playback started; a seek to 50:00 got its first
  bytes after 0.49 s.
- From a bare `magnet:?xt=urn:btih:<hash>` (what Torrentio gives): metadata
  via DHT in 4.5 s, first bytes 1.5 s later, a seek to 30:00 in 0.67 s,
  7–11 MiB/s.

`librqbit` facts (VERIFIED from the 9.0.1 source):
- Apache-2.0. `ManagedTorrent::stream(file_id)` returns a `FileStream` that
  implements `AsyncRead` + `AsyncSeek` and prioritizes the pieces being read.
- Storage is pluggable (`StorageFactory`/`TorrentStorage`), so file names
  from the torrent need not become filesystem paths.
- Outgoing peer connections are checked against a blocklist loaded from a
  `file://` URL. With `listen: None` it accepts no incoming connections.
  Local service discovery can be disabled.
- HTTP tracker announces use its own `reqwest` client, not `cineo-net`.
- Its own tests stream between two local sessions without internet access
  (`src/tests/e2e_stream.rs`), so ours can too.
- `librqbit-core` depends on `directories` → `dirs-sys` → `option-ext`
  (MPL-2.0), which is not in our license allowlist.

`cratetorrent` 0.1.0 is unmaintained since 2020 (ADR-0010).

## Options considered
1. **`librqbit` as a library** inside `cineo-stream`, with our own loopback
   HTTP server, storage and peer blocklist.
2. `librqbit` with its `http-api` feature (axum) serving files. Less code,
   but its routes, tokens and host checks are not ours to control.
3. Write a BitTorrent client. Months of work for no user-visible gain.

## Decision
- Option 1. `cineo-stream` depends on `librqbit` 9.0.1
  (`default-features = false`, `rust-tls`) and serves files with our own
  `hyper` server: loopback only, a random per-session path token, a `Host`
  check, single byte ranges, no CORS.
- **Network policy exception (amends ADR-0006).** BitTorrent traffic (peer
  connections, DHT, tracker announces) goes through `librqbit`, not
  `cineo-net`. Controls instead of `NetPolicy`:
  - No incoming connections; no UPnP.
  - Unless the user allowed private networks, outgoing peer connections to
    non-public addresses are blocked (generated blocklist), local service
    discovery is off, and trackers with a literal non-public IP are dropped.
  - Tracker host names are not resolved through our resolver (residual risk,
    recorded in SECURITY.md and TECHNICAL_DEBT.md).
  - P2P can be switched off entirely; nothing torrent-related starts until
    the first torrent stream is played.
- **Uploading.** Cineo uploads to the peers it is connected to while a
  torrent streams, as BitTorrent clients and Stremio do (owner decision
  2026-10-06). The P2P notice says so.
- **Licensing.** `option-ext` (MPL-2.0) is allowed as a single-crate
  exception in `deny.toml` (owner decision 2026-10-06). MPL-2.0 is
  file-level copyleft; we use it unmodified.
- **Files on disk.** Our storage names files `<cache>/<info hash>/<file
  index>`; torrent file names never become paths.

## Consequences
- AGENTS.md's network invariant gains this one exception.
- The cache, the HTTP server and the blocklist need their own tests.
- `librqbit` adds about 350 crates to the build (most shared with reqwest
  and tokio) and is a large surface we do not control; its updates need
  review like any network-facing dependency.
- DHT UDP traffic is not filtered by our blocklist (UNKNOWN whether
  `librqbit` filters it); recorded as technical debt.

## Rejected alternatives
- Option 2: the HTTP surface would not follow SECURITY.md (token, Host
  check), and it pulls in axum.
- Option 3: no benefit over a maintained, Apache-2.0 library that passed
  the spike.
