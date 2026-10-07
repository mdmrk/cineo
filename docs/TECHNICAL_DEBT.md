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

### Subtitles: no video hash or size from the player
- Where: [state.rs](../crates/cineo-core/src/app/state.rs) (`subtitle_groups`)
- Gap: the `subtitles` request's `videoHash`, `videoSize` and `filename`
  come only from the stream's `behaviorHints`. The reference client also
  uses values its player reports (an OpenSubtitles hash from its streaming
  server, the file size and name).
- Why: computing the hash needs the first and last 64 KiB of the file
  (from `cineo-stream` for torrents, or range requests for HTTP), which is
  not built yet.
- Instead: for streams without hints, subtitle addons are asked by video id
  only, so hash-matched results (exact sync) are missing.
- Exit: report the file size and name from the player or the engine, and
  compute the OpenSubtitles hash in `cineo-stream`.

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
- Instead: it is paused, so it moves no data; its files are deleted when
  the engine next starts.
- Exit: have `open` record the torrent id before awaiting anything else,
  or remove unknown torrents from the session at the next `open`.

### Streaming engine: the torrent being played can fill the disk
- Where: [storage.rs](../crates/cineo-stream/src/storage.rs)
- Gap: the file being played is kept whole on disk until it stops, so a
  file larger than the free space fills the disk.
- Why: deleting data under the player would break playback and seeking.
- Instead: nothing else is kept: data is deleted when the torrent stops
  and leftovers when the engine starts.
- Exit: refuse files larger than the free space up front, or drop pieces
  already played (needs piece-level storage).

### Streaming engine: shared urgent pieces waste some bandwidth and blame
- Where: `crates/librqbit/src/piece_tracker.rs` in our librqbit fork
  ([commit be52c57a](https://github.com/mdmrk/rqbit/commit/be52c57a0d6c85d6d35467a2c8d5d60f66924aea), ADR-0016)
- Gap: up to three peers download each of the next four pieces a stream
  needs. When one finishes, the others are not sent BitTorrent `cancel`
  messages, so their chunks still arrive and are dropped. If a shared
  piece fails its hash check, the peer that completed it is disconnected,
  though a helper may have sent the bad chunk (INFERRED from the code).
- Why: upstream librqbit sends no cancellations when a piece completes
  either; tracking which peer wrote each chunk is a larger change.
- Instead: the waste is bounded (four pieces, two helpers each); a failed
  piece is downloaded again.
- Exit: send `cancel` to helpers on completion; remember the writer of
  each chunk of a shared piece.

### Streaming engine: ringbuf advisory ignored
- Where: `librqbit-utp` 0.7.0 → `ringbuf` 0.4.8; `.config/deny.toml`
- Gap: RUSTSEC-2026-0293 (double free when an element's `Drop` panics) is
  ignored; the fix is in `ringbuf` 0.5.2, which `librqbit-utp` does not use.
- Why: no newer `librqbit-utp` (0.7.0 is the latest, 2026-10-07).
- Instead: not reachable (VERIFIED from its source): it stores only `u8`,
  which has no `Drop`.
- Exit: drop the ignore once `librqbit-utp` moves to `ringbuf` 0.5.2+.

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

### Desktop UI: the font licenses are not packaged in tagged releases
- Where: [release.yml](../.github/workflows/release.yml),
  [assets/fonts](../crates/cineo-desktop/assets/fonts)
- Gap: the OFL-1.1 texts of the bundled fonts and the Tabler MIT notice
  from `iconflow` must ship with any `cineo-desktop` binary. The rolling
  `prerelease` build ([prerelease.yml](../.github/workflows/prerelease.yml))
  copies them; tagged releases still ship only the `cineo` CLI.
- Why: desktop packaging for tagged releases is M7.
- Instead: the licenses sit next to the font files in the source tree.
- Exit: when tagged releases package the desktop app (M7), copy the same
  files as `prerelease.yml` does.

### Desktop UI: no app icon on Wayland
- Where: [brand.rs](../crates/cineo-desktop/src/brand.rs), `app::run`
- Gap: the window icon is set through `ViewportBuilder::with_icon`.
  Wayland has no protocol for an app to set its own window icon; the
  compositor takes it from a `cineo.desktop` entry matching the `cineo`
  app id. Nothing installs that entry yet, so on Wayland the taskbar shows
  a generic icon (INFERRED from winit's platform notes; UNKNOWN on Windows
  and macOS, which are untested).
- Why: no desktop packaging yet.
- Instead: X11 and the other platforms get the icon from the window.
- Exit: when desktop packaging lands (M7), install `cineo.desktop` and the
  icon under the hicolor theme.

### Desktop UI: rows scroll sideways only with Shift, a touchpad or arrows
- Where: [view.rs](../crates/cineo-desktop/src/view.rs) (`poster_strip`)
- Gap: a plain mouse wheel over a poster row scrolls the page, not the row.
- Why: egui gives the wheel to one scroll area; the page wins so vertical
  browsing never gets stuck on a row.
- Instead: arrow buttons appear on hover; Shift+wheel and touchpads scroll
  rows directly.
- Exit: none planned; this is the intended behavior unless users ask
  otherwise.

### Desktop UI: image textures are never evicted
- Where: [images.rs](../crates/cineo-desktop/src/images.rs)
- Gap: every poster and backdrop shown stays on the GPU until the app
  quits; egui's texture cache only drops unused sizes of SVGs.
- Why: eviction needs to know which images were drawn recently, which
  egui does not expose to image loaders.
- Instead: GPU memory grows with the number of distinct images seen in a
  session (the RAM copy is dropped after upload).
- Exit: track the image URIs painted each frame and `forget_image` those
  unused for a while.

### Desktop UI: off-screen stream cards are not in the accessibility tree
- Where: [view.rs](../crates/cineo-desktop/src/view.rs) (`stream_group`)
- Gap: stream cards outside the scrolled view are skipped, so screen readers
  see only the cards on screen.
- Why: laying out every card made each frame of a 300-stream list take
  3.2 ms instead of 0.15 ms.
- Instead: scrolling brings the other cards into the tree.
- Exit: report skipped cards to AccessKit with their remembered rect.

### Player settings: pause on minimize depends on frames while minimized
- Where: [app.rs](../crates/cineo-desktop/src/app.rs) (`pause_on_minimize`)
- Gap: the check runs in eframe's `logic`, once per frame. Whether frames
  still run while the window is minimized is UNKNOWN on each platform
  (Wayland compositors may stop them).
- Why: eframe reports `minimized` through viewport info only.
- Instead: if no frame runs after minimizing, the video keeps playing until
  the window is shown again, and then it pauses.
- Exit: verify per platform; otherwise react to the window event directly.

