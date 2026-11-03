# Known limitations

Limitations of the released software. Each entry says:
- what is affected (with a link to code),
- what does not work,
- why,
- what happens instead,
- what would remove the limitation.

Fix an entry and delete it in the same change.

Format:

```
### <Area>: <short title>
- Where: [path](../path)
- Gap: …
- Why: …
- Instead: … (observable behavior today)
- Exit: … (what would fix it)
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
  not built.
- Instead: for streams without hints, subtitle addons are asked by video id
  only, so hash-matched results (exact sync) are missing.
- Exit: report the file size and name from the player or the engine, and
  compute the OpenSubtitles hash in `cineo-stream`.

### Embedded player: subtitles under the controls
- Where: [player.rs](../crates/cineo-desktop/src/player.rs)
- Gap: subtitles stay at their position while the controls are shown and
  can sit under the bottom bar.
- Why: moving subtitles needs a `sub-margin-y` change tied to the
  controls' visibility, which was not built.
- Instead: subtitles can be covered for up to 2.5 s after input.
- Exit: adjust `sub-margin-y` when the controls show and hide.

### Embedded player: VA-API interop not seen working
- Where: [render.rs](../crates/cineo-player/src/embedded/render.rs)
- Gap: the render context now gets the window's Wayland or Xlib display,
  which mpv's VA-API interop needs; that it then decodes in hardware is
  UNKNOWN (not run). XCB-only X11 sessions get no display.
- Exit: check `hwdec-current` in the stats overlay on Intel/AMD under
  Wayland and X11.

### Embedded player: GL thread assumption
- Where: [app.rs](../crates/cineo-desktop/src/app.rs)
- Gap: the renderer is created and freed inside eframe's `logic`/`ui`/
  `on_exit`, which assumes the window's GL context is current there
  (INFERRED from eframe's single-window glow integration). A test build
  that created it at startup and freed it in `ui` worked under Wayland and
  X11 (VERIFIED 2026-10-07).
- Why: eframe exposes the GL context only inside those callbacks.
- Instead: it works on the tested sessions; other GL drivers are UNKNOWN.
- Exit: create and free the renderer from a callback eframe documents as
  having the context current.

### Streaming engine: UDP trackers and the DHT bypass the address filter
- Where: [engine.rs](../crates/cineo-stream/src/engine.rs)
- Gap: a UDP tracker given by host name may resolve to a private address
  and still be contacted; this includes the eight built-in fallback
  trackers (ADR-0026). DHT traffic (UDP) is not filtered either; the peer
  addresses it returns are (ADR-0026).
- Why: SOCKS5 `CONNECT` covers TCP only, and librqbit 9.0.1 sends UDP
  tracker and DHT packets from its own sockets (INFERRED from its source:
  `librqbit-tracker-comms` `tracker_comms_udp.rs`).
- Instead: HTTP trackers are filtered by the proxy, peer addresses by
  librqbit's blocklist; trackers on a literal non-public IP are dropped
  from the magnet.
- Exit: resolve UDP tracker host names ourselves and drop non-public ones,
  or an upstream hook for UDP destinations; for the DHT, an upstream
  address filter.

### Streaming engine: a cancelled open can leave a torrent running
- Where: [app.rs](../crates/cineo-desktop/src/app.rs) (`start_torrent`),
  [engine.rs](../crates/cineo-stream/src/engine.rs) (`open`, `stop`)
- Gap: a stop or a new start cancels an `open` still in progress. If that
  happens after librqbit added the torrent, the torrent stays in the
  session until the next `open` or `stop`, which delete every torrent in
  the session. With a window it is added unpaused, so until then it keeps
  peer connections and DHT and tracker lookups, but downloads no piece
  because no stream reads it (INFERRED from the code; not observed).
  Without a window it stays paused.
- Why: cancelling is what keeps a stuck metadata lookup from blocking the
  next torrent; not pausing keeps the peers found for the metadata.
- Instead: the next `open` or `stop` removes it and its files.
- Exit: delete the torrent from the session when the `open` is cancelled.

### Streaming engine: torrent data can exceed its window
- Where: [memory.rs](../crates/cineo-stream/src/memory.rs), [storage.rs](../crates/cineo-stream/src/storage.rs) and
  `torrent_state/live/mod.rs` in our librqbit fork
  ([commit b815c324](https://github.com/mdmrk/rqbit/commit/b815c324922179f7693f506bb9499430bedd9907), ADR-0019)
- Gap: only validated pieces count toward the window. Pieces in flight
  come on top, and a partial piece whose peer died and that no stream
  needs again stays in memory or on disk until the torrent stops. A piece forgotten
  while it waits in a peer's upload queue cannot be read, and that peer is
  disconnected (INFERRED from the code).
- Why: librqbit tells the storage nothing when a piece is abandoned, and
  its upload path treats a failed read as fatal for the connection.
- Instead: in-flight pieces are a few per peer; abandoned ones are
  usually near the playback position and get downloaded again. A
  disconnected peer is retried with backoff.
- Exit: a librqbit hook for abandoned pieces, and skipping unreadable
  upload requests instead of disconnecting.

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
- Exit: a GUI prompt that renames the file aside and starts fresh.

### Desktop: no log file or diagnostics view
- Where: [main.rs](../crates/cineo-desktop/src/main.rs)
- Gap: logs go to stderr only; there is no log file and no diagnostics
  view.
- Why: it would add a dependency (`tracing-appender`) and a page.
- Instead: start `cineo-desktop -v` from a terminal; `cineo doctor` covers
  the database.
- Exit: a log file in `$XDG_STATE_HOME/cineo`, plus a diagnostics page.

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
- Gap: self-hosted addons need `--allow-private-network` or the "Allow
  local network addresses" setting, which applies to every addon; there
  is no per-addon trust.
- Why: per-addon trust needs a prompt at install time and a stored flag.
- Instead: blocked addons fail to install with "blocked by network policy".
- Exit: a trust flag set at install time with a clear prompt.

### Store: manifests are not cached
- Where: [migrate.rs](../crates/cineo-store/src/migrate.rs) (only
  transport URLs are stored)
- Gap: at startup every installed addon's manifest is fetched again; an
  addon that is offline is reported and missing until the next start.
- Why: the core's `Restore` action takes URLs only.
- Instead: `State.notice` reports the addon that failed to load.
- Exit: store the last good manifest and restore from it.

### Addon lists: only lists of type `all`, no logos
- Where: [plan.rs](../crates/cineo-core/src/app/plan.rs) (`addon_list_targets`),
  [view.rs](../crates/cineo-desktop/src/view.rs) (`addon_lists`)
- Gap: `addonCatalogs` of other types (Cinemeta also has `movie`, `series`,
  `channel`, …) are not offered, there is no search in a list, and listed
  addons show no logo.
- Why: it mirrors Stremio's default view; Cinemeta's `all`
  lists already contain every addon of its typed lists (VERIFIED against
  the live lists, 2026-10-10).
- Instead: one chip per `all` list; each addon shows name, version, host,
  description and badges.
- Exit: a type filter over the same `addonCatalogs`, and logos through the
  existing image loader.

### Desktop UI: only Latin and Cyrillic, left-to-right UI languages
- Where: [i18n.rs](../crates/cineo-desktop/src/i18n.rs),
  [assets/locales](../crates/cineo-desktop/assets/locales)
- Gap: the UI is offered in English, Spanish (Spain and Latin America),
  Russian, Italian, German, French and Brazilian Portuguese only. Portugal
  gets Brazilian Portuguese.
  Right-to-left languages and scripts outside the bundled fonts cannot be
  added.
  Numbers are formatted the same way in every locale (`1.5`, not `1,5`).
  Counted messages are worded as "label: number" because the catalogs
  have no plural rules (ADR-0021).
  Technical details from IO layers stay in English inside a translated
  sentence.
- Why: egui does not shape right-to-left text, the bundled fonts cover
  Latin and Cyrillic only, and `rust-i18n` has neither plural rules nor
  per-locale number formatting.
- Instead: other system locales fall back to English.
- Exit: bundle fonts for more scripts, wait for bidi support in egui, and
  give IO errors typed kinds that the UI can word.

### Desktop UI: no app icon on Wayland
- Where: [brand.rs](../crates/cineo-desktop/src/brand.rs), `app::run`
- Gap: the window icon is set through `ViewportBuilder::with_icon`.
  Wayland has no protocol for an app to set its own window icon; the
  compositor takes it from a `cineo.desktop` entry matching the `cineo`
  app id. `install.sh` installs that entry and the icon under the hicolor
  theme; a binary started any other way shows a generic icon on Wayland
  (INFERRED from winit's platform notes).
- Why: there is no Wayland protocol for it.
- Instead: X11 gets the icon from the window.
- Exit: none; install with `install.sh`.

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

### Desktop UI: off-screen stream cards and listed addons are not in the accessibility tree
- Where: [view.rs](../crates/cineo-desktop/src/view.rs) (`stream_group`,
  `addon_lists`)
- Gap: stream cards and addon list rows outside the scrolled view are
  skipped, so screen readers see only the ones on screen.
- Why: laying out every card made each frame of a 300-stream list take
  3.2 ms instead of 0.15 ms.
- Instead: scrolling brings the others into the tree.
- Exit: report skipped rows to AccessKit with their remembered rect.

### Desktop UI: stream facts are keyed by the list's address
- Where: [view.rs](../crates/cineo-desktop/src/view.rs) (`StreamFacts`)
- Gap: the quality, seeders, size, search text and sort order of each
  addon's streams are computed once and reused while the list keeps the
  same address and length.
- Why: the core's `Vec<Stream>` has no version or identity to compare, and
  rebuilding the facts every frame cost several allocations per stream.
- Instead: if a list were replaced between two frames by one of the same
  length at the same address, the old facts would stay until the detail
  page is left. Stream indexes stay valid, so Play still picks the stream
  shown at that index.
- Exit: a generation number or `Arc` for each stream group in the core.

### Player settings: pause on minimize depends on frames while minimized
- Where: [app.rs](../crates/cineo-desktop/src/app.rs) (`pause_on_minimize`)
- Gap: the check runs in eframe's `logic`, once per frame. Whether frames
  still run while the window is minimized is UNKNOWN per compositor
  (Wayland compositors may stop them).
- Why: eframe reports `minimized` through viewport info only.
- Instead: if no frame runs after minimizing, the video keeps playing until
  the window is shown again, and then it pauses.
- Exit: react to the window event directly.

### Deep links: plain second launches
- Where: [instance.rs](../crates/cineo-desktop/src/instance.rs)
- Gap: a second launch without a link still opens a second window.
- Why: only links needed a handover.
- Instead: second windows share the database.
- Exit: hand plain launches to the running window too, which raises it.

### Windows: nothing has run on a Windows machine
- Where: every `cfg(windows)` path (ADR-0027)
- Gap: CI compiles and tests on `windows-latest`, and the app starts under
  wine, but no one has started it on real Windows. Playback with the embedded libmpv over the OpenGL
  render API, `hwdec=auto-safe`, the named pipe, `reg.exe` registration,
  SMTC, keep-awake and `install.ps1` are UNKNOWN.
- Why: there is no Windows machine in development.
- Instead: Windows is marked Untested in COMPATIBILITY.md.
- Exit: run the [manual test](DEVELOPMENT.md#manual-gui-test) on Windows
  10 and 11 and record the results.

### Windows: the bundled libmpv depends on an upstream release
- Where: [fetch-libmpv.sh](../scripts/fetch-libmpv.sh),
  [libmpv-windows.env](../scripts/libmpv-windows.env)
- Gap: the Windows build downloads the pinned archive from
  zhongfly/mpv-winbuild only. Each Cineo release carries a copy, but the
  build does not fall back to it.
- Why: no separate mirror release or tag (owner decision 2026-11-03).
- Instead: if upstream deletes that release, the Windows build fails at
  the checksum step until the pin moves to a newer build.
- Exit: none planned; update the pin when it breaks.

### Windows: the bundled libmpv reads HTTP through libcurl
- Where: the pinned build in [libmpv-windows.env](../scripts/libmpv-windows.env)
- Gap: this mpv build opens `http(s)` with libcurl, not FFmpeg. Against a
  server without `Accept-Ranges`, it treats the stream as not seekable:
  the duration is estimated and resume and seeking fail. FFmpeg (libmpv
  0.41 on Linux) tries a range request anyway. Seen under wine,
  2026-11-03. `--curl-enabled=no` still opened the stream with libcurl.
- Why: zhongfly builds mpv with `-Dlibcurl=enabled`.
- Instead: Cineo's own torrent server sends `Accept-Ranges`, and so do
  the test server and the usual CDNs; an addon stream from a server that
  does not is not seekable on Windows.
- Exit: find the mpv option that falls back to FFmpeg's HTTP, or pin a
  build without libcurl.

### Windows: private files rely on the profile's ACL
- Where: `create_private_dir` in `cineo-store` and `cineo-stream`,
  `subtitles.rs`
- Gap: on Unix these directories and files get mode 0700/0600; on Windows
  they inherit the ACL of `%APPDATA%` or `%LOCALAPPDATA%`, which is private
  to the user by default but not checked.
- Why: setting an explicit ACL needs `unsafe` Win32 calls.
- Instead: a changed profile ACL is inherited as is.
- Exit: set an owner-only DACL on creation and test it on Windows.

### Desktop: the screen stays awake only through `org.freedesktop.ScreenSaver`
- Where: [keep_awake.rs](../crates/cineo-desktop/src/keep_awake.rs)
- Gap: while a video plays, Cineo inhibits the screen saver only over the
  session D-Bus `org.freedesktop.ScreenSaver` interface. There is no
  Wayland `idle-inhibit` and no XDG portal `Inhibit`, and nothing tests it
  against a real desktop.
- Why: the interface covers KDE Plasma and GNOME with one D-Bus call. With
  the libmpv render API, mpv does not do this itself (INFERRED).
- Instead: on a desktop without the interface, a warning is logged and the
  screen may sleep during playback.
- Exit: add the Wayland `idle-inhibit` protocol on the window surface, and
  test it on the desktops Cineo targets.

### Desktop: MPRIS covers play, pause and stop only
- Where: [mpris.rs](../crates/cineo-desktop/src/mpris.rs)
- Gap: the MPRIS service offers play, pause, stop and raise, with the title
  as metadata. It has no seek, position, volume, next or previous, and
  nothing tests it against a real desktop or a real headset.
- Why: media keys on headphones only needed play and pause.
- Instead: next and previous do nothing, and the desktop shows no
  progress bar for Cineo.
- Exit: add `Position`, `Seek`, `SetPosition`, `Volume` and next episode,
  and a test that talks to the service over a private bus.
