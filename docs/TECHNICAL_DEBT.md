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
- Where: [path](../path)
- Gap: …
- Why: …
- Instead: … (observable behavior today)
- Exit: … (what would fix it; link issue/milestone)
```

## Entries

### Embedded player: no hardware-decoding interop display, subtitles under the controls
- Where: [render.rs](../crates/cineo-player/src/embedded/render.rs),
  [player.rs](../crates/cineo-desktop/src/player.rs)
- Gap: the render context gets no `X11_DISPLAY`/`WL_DISPLAY` parameter, so
  mpv may not use zero-copy hardware decoding (VA-API interop). Subtitles
  stay at their position while the controls are shown and can sit under
  the bottom bar.
- Why: passing the display needs the window's raw display handle from
  eframe; moving subtitles needs a `sub-margin-y` change tied to the
  controls' visibility. Both were left out of the first slice.
- Instead: mpv decodes with a copy-back or software path (UNKNOWN which on a
  given machine); subtitles can be covered for up to 2.5 s after input.
- Exit: pass the raw display handle at render-context creation; adjust
  `sub-margin-y` when the controls show and hide.

### Embedded player: GL thread assumption, Windows and macOS untested
- Where: [app.rs](../crates/cineo-desktop/src/app.rs)
- Gap: the renderer is created and freed inside eframe's `logic`/`ui`/
  `on_exit`, which assumes the window's GL context is current there
  (INFERRED from eframe's single-window glow integration). On Linux, a test
  build that created it at startup and freed it in `ui` worked under
  Wayland and X11 (VERIFIED 2026-10-07); the full app path is not yet
  exercised. Windows and macOS have never run embedded playback.
- Why: no Windows or macOS machine in the loop yet.
- Instead: on Linux it works; elsewhere behavior is UNKNOWN.
- Exit: run the manual test on Windows and macOS (M7).

### Streaming engine: UDP trackers and the DHT bypass the address filter
- Where: [engine.rs](../crates/cineo-stream/src/engine.rs)
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

### Streaming engine: a cancelled open can leave a paused torrent
- Where: [app.rs](../crates/cineo-desktop/src/app.rs) (`start_torrent`),
  [engine.rs](../crates/cineo-stream/src/engine.rs) (`open`)
- Gap: a stop or a new start cancels an `open` still in progress. If that
  happens after librqbit added the torrent but before the engine recorded
  it, the torrent stays in the session, paused, until the app exits
  (INFERRED from the code; not observed).
- Why: cancelling is what keeps a stuck metadata lookup from blocking the
  next torrent.
- Instead: it is paused, so it moves no data; its files stay in the cache.
- Exit: have `open` record the torrent id before awaiting anything else,
  or remove unknown torrents from the session at the next `open`.

### Streaming engine: the torrent being played can exceed the cache limit
- Where: [cache.rs](../crates/cineo-stream/src/cache.rs)
- Gap: eviction runs when a torrent opens and never removes the current
  one, so a file larger than the limit fills the disk past it.
- Why: deleting data under the player would break playback.
- Instead: other torrents are evicted first; the current one keeps growing.
- Exit: refuse files larger than the limit up front, or evict pieces
  already played (needs piece-level storage).

### Store: no in-app recovery from a corrupt database
- Where: [store.rs](../crates/cineo-store/src/store.rs)
- Gap: a corrupt `cineo.db` makes every persistence command fail.
- Why: silently replacing it would lose the user's library.
- Instead: the error and `cineo doctor` explain it; the user moves the file
  away and Cineo starts fresh.
- Exit: a GUI prompt (M5) that renames the file aside and starts fresh.

### Desktop: no log file or diagnostics view
- Where: [main.rs](../crates/cineo-desktop/src/main.rs)
- Gap: logs go to stderr only; the debugging guide planned a rotating file and an
  About/Diagnostics view.
- Why: it would add a dependency (`tracing-appender`) and a page outside
  M5's acceptance.
- Instead: start `cineo-desktop -v` from a terminal; `cineo doctor` covers
  the database.
- Exit: a log file in the platform state directory, plus a diagnostics page.

### Desktop: image cache is unbounded for a session
- Where: [images.rs](../crates/cineo-desktop/src/images.rs)
- Gap: decoded images (downscaled to about twice their display size) stay
  in memory until exit.
- Why: egui decides when to forget images; a size-bounded cache was not
  needed for normal browsing.
- Instead: memory grows with the number of distinct posters viewed.
- Exit: an LRU cap on `NetImageLoader` images.

### Desktop: private networks are a launch flag only
- Where: [main.rs](../crates/cineo-desktop/src/main.rs)
- Gap: self-hosted addons need `--allow-private-network`; there is no
  setting, and no per-addon trust (ROADMAP.md, v0.x).
- Why: a settings page was not in M5's scope.
- Instead: blocked addons fail to install with "blocked by network policy".
- Exit: a settings page, then per-addon trust.

### Store: manifests are not cached
- Where: [migrate.rs](../crates/cineo-store/src/migrate.rs) (only
  transport URLs are stored)
- Gap: at startup every installed addon's manifest is fetched again; an
  addon that is offline is reported and missing until the next start.
- Why: the core's `Restore` action takes URLs only (M2); caching was not in
  M4's scope.
- Instead: `State.notice` reports the addon that failed to load.
- Exit: store the last good manifest and restore from it (v0.x caching).

### Desktop UI: the font licenses are not packaged yet
- Where: [release.yml](../.github/workflows/release.yml),
  [assets/fonts](../crates/cineo-desktop/assets/fonts)
- Gap: the OFL-1.1 texts of the bundled fonts must ship with any
  `cineo-desktop` binary; no release packages the desktop app yet, so
  nothing copies `Inter-OFL.txt` and `DMSerifDisplay-OFL.txt`, nor the
  Tabler MIT notice that comes with `iconflow`.
- Why: releases currently ship only the `cineo` CLI.
- Instead: the licenses sit next to the font files in the source tree.
- Exit: when desktop packaging lands (M7), copy both `*-OFL.txt` files
  into every desktop archive or installer.

### Desktop UI: rows scroll sideways only with Shift, a touchpad or arrows
- Where: [view.rs](../crates/cineo-desktop/src/view.rs) (`poster_strip`)
- Gap: a plain mouse wheel over a poster row scrolls the page, not the row.
- Why: egui gives the wheel to one scroll area; the page wins so vertical
  browsing never gets stuck on a row.
- Instead: arrow buttons appear on hover; Shift+wheel and touchpads scroll
  rows directly.
- Exit: none planned; this is the intended behavior unless users ask
  otherwise.
