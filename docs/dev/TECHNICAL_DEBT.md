# Technical debt and known gaps

Known limitations that are **intentional for now**. Each entry says:
- what is affected (with a link to code),
- what does not work,
- why,
- what happens instead,
- what would remove the limitation.

Fix an entry and delete it in the same change. Do not hide debt in `TODO`
comments without an entry here.

Format:

```
### <Area>: <short title>
- Where: [path](../../path)
- Gap: …
- Why: …
- Instead: … (observable behavior today)
- Exit: … (what would fix it; link issue/milestone)
```

## Entries

### Streaming engine: UDP trackers and the DHT bypass the address filter
- Where: [engine.rs](../../crates/cineo-stream/src/engine.rs)
- Gap: a UDP tracker given by host name may resolve to a private address
  and still be contacted; DHT traffic (UDP) is not filtered either.
- Why: SOCKS5 `CONNECT` covers TCP only, and librqbit 9.0.1 sends UDP
  tracker and DHT packets from its own sockets (INFERRED from its source:
  `librqbit-tracker-comms` `tracker_comms_udp.rs`).
- Instead: peer TCP connections and HTTP trackers are filtered by the
  proxy; trackers on a literal non-public IP are dropped from the magnet.
- Exit: resolve UDP tracker host names ourselves and drop non-public ones,
  or an upstream hook for UDP destinations; for the DHT, an upstream
  address filter.

### Streaming engine: the torrent being played can exceed the cache limit
- Where: [cache.rs](../../crates/cineo-stream/src/cache.rs)
- Gap: eviction runs when a torrent opens and never removes the current
  one, so a file larger than the limit fills the disk past it.
- Why: deleting data under the player would break playback.
- Instead: other torrents are evicted first; the current one keeps growing.
- Exit: refuse files larger than the limit up front, or evict pieces
  already played (needs piece-level storage).

### Store: no in-app recovery from a corrupt database
- Where: [store.rs](../../crates/cineo-store/src/store.rs)
- Gap: a corrupt `cineo.db` makes every persistence command fail.
- Why: silently replacing it would lose the user's library.
- Instead: the error and `cineo doctor` explain it; the user moves the file
  away and Cineo starts fresh.
- Exit: a GUI prompt (M5) that renames the file aside and starts fresh.

### Desktop: no log file or diagnostics view
- Where: [main.rs](../../crates/cineo-desktop/src/main.rs)
- Gap: logs go to stderr only; DEBUGGING.md planned a rotating file and an
  About/Diagnostics view.
- Why: it would add a dependency (`tracing-appender`) and a page outside
  M5's acceptance.
- Instead: start `cineo-desktop -v` from a terminal; `cineo doctor` covers
  the database.
- Exit: a log file in the platform state directory, plus a diagnostics page.

### Desktop: image cache is unbounded for a session
- Where: [images.rs](../../crates/cineo-desktop/src/images.rs)
- Gap: fetched image bytes (≤ 4 MiB each) stay in memory until exit.
- Why: egui decides when to forget images; a size-bounded cache was not
  needed for normal browsing.
- Instead: memory grows with the number of distinct posters viewed.
- Exit: an LRU cap on `NetImageLoader` bytes.

### Desktop: private networks are a launch flag only
- Where: [main.rs](../../crates/cineo-desktop/src/main.rs)
- Gap: self-hosted addons need `--allow-private-network`; there is no
  setting, and no per-addon trust (GOALS.md v0.x).
- Why: a settings page was not in M5's scope.
- Instead: blocked addons fail to install with "blocked by network policy".
- Exit: a settings page, then per-addon trust.

### Store: manifests are not cached
- Where: [migrate.rs](../../crates/cineo-store/src/migrate.rs) (only
  transport URLs are stored)
- Gap: at startup every installed addon's manifest is fetched again; an
  addon that is offline is reported and missing until the next start.
- Why: the core's `Restore` action takes URLs only (M2); caching was not in
  M4's scope.
- Instead: `State.notice` reports the addon that failed to load.
- Exit: store the last good manifest and restore from it (v0.x caching).
