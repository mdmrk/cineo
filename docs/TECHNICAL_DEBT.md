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
  [addons.rs](../crates/cineo-desktop/src/view/addons.rs) (`addon_lists`)
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

### Desktop UI: no right-to-left UI languages, and Chinese needs an installed font
- Where: [i18n.rs](../crates/cineo-desktop/src/i18n.rs),
  [assets/locales](../crates/cineo-desktop/assets/locales)
- Gap: the UI is offered in English, Spanish (Spain and Latin America),
  Russian, Italian, German, French, Brazilian Portuguese and Simplified
  Chinese only. Portugal gets Brazilian Portuguese, and Traditional Chinese
  systems get Simplified Chinese. Simplified Chinese was translated on
  2026-11-08 and has not been reviewed by a native speaker.
  Chinese glyphs are not bundled: without a CJK font installed, Chinese text,
  and the name of the language in the picker, draw as boxes.
  Right-to-left languages cannot be added.
  Numbers are formatted the same way in every locale (`1.5`, not `1,5`).
  Counted messages are worded as "label: number" because the catalogs
  have no plural rules (ADR-0021).
  Technical details from IO layers stay in English inside a translated
  sentence.
- Why: egui does not shape right-to-left text, the bundled fonts cover
  Latin and Cyrillic only (a CJK font would add megabytes), and `rust-i18n` has neither plural rules nor
  per-locale number formatting.
- Instead: other system locales fall back to English.
- Exit: wait for bidi support in egui, and
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
- Where: [cards.rs](../crates/cineo-desktop/src/view/cards.rs) (`poster_strip`)
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
- Where: [view/](../crates/cineo-desktop/src/view) (`stream_group`,
  `addon_lists`)
- Gap: stream cards and addon list rows outside the scrolled view are
  skipped, so screen readers see only the ones on screen.
- Why: laying out every card made each frame of a 300-stream list take
  3.2 ms instead of 0.15 ms.
- Instead: scrolling brings the others into the tree.
- Exit: report skipped rows to AccessKit with their remembered rect.

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
  render API, `hwdec=auto-safe`, the loopback link handover, `reg.exe` registration,
  SMTC, keep-awake and `install.ps1` are UNKNOWN.
- Why: there is no Windows machine in development.
- Instead: Windows is marked Untested in COMPATIBILITY.md.
- Exit: run the [manual test](DEVELOPMENT.md#manual-gui-test) on Windows
  10 and 11 and record the results.

### Windows: the bundled libmpv depends on an upstream release
- Where: [fetch-libmpv.sh](../scripts/fetch-libmpv.sh),
  [libmpv-windows.env](../scripts/libmpv-windows.env)
- Gap: the Windows build downloads the pinned archive from
  zhongfly/mpv-winbuild only; Cineo keeps no copy, in releases or
  elsewhere.
- Why: releases carry only what users install, and no separate mirror
  release or tag (owner decisions 2026-11-03).
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
  `subtitles.rs` in `cineo-player`
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
  Wayland `idle-inhibit` and no XDG portal `Inhibit` for the screen, and
  nothing tests it against a real desktop. `fastframe-power` only blocks idle
  system sleep, not the screen turning off.
- Why: the interface covers KDE Plasma and GNOME with one D-Bus call. With
  the libmpv render API, mpv does not do this itself (INFERRED).
- Instead: on a desktop without the interface, a warning is logged and the
  screen may sleep during playback.
- Exit: add the Wayland `idle-inhibit` protocol on the window surface, and
  test it on the desktops Cineo targets.

### Desktop: egui is held at 0.36.2 for fastframe
- Where: [Cargo.toml](../Cargo.toml) (ADR-0033)
- Gap: Cineo is one egui release behind. It misses 0.37's accessibility
  improvements and fixes. Emoji, other scripts and scrolling come from
  fastframe instead, and that wiring, the font settings and the graphics
  dialog have no test on a real desktop.
- Why: fastframe's egui crates need 0.36, and it has no move to 0.37 yet.
- Instead: text and scrolling follow fastframe's behavior, not egui's.
- Exit: move to the egui release fastframe adopts.

### Desktop: MPRIS has no next or previous
- Where: [media_keys.rs](../crates/cineo-desktop/src/media_keys.rs)
- Gap: the media controls offer play, pause, stop, raise, seek, position and
  volume, with the title and length as metadata. They have no next or
  previous, and nothing tests them against a real desktop or a real headset.
- Why: next episode needs the player's next-up state in the controls.
- Instead: next and previous are shown as unavailable.
- Exit: add next episode, and a test that talks to the service over a
  private bus.

### Android: the language follows the system only
- Where: [res](../android/app/src/main/res) (`values-*/strings.xml`)
- Gap: the Android UI has the desktop's nine languages, but no Language
  setting: Android picks the strings from the system language. Its texts
  are a copy of the desktop's translations (`crates/cineo-desktop/assets/locales/`)
  plus 24 strings and 2 plurals of its own, translated on 2026-11-05 and
  not reviewed by a native speaker; nothing keeps the two copies in step.
- Why: the desktop's catalogs are compiled into the Rust UI; Android needs
  its own string resources.
- Instead: a user whose system language is not one of the nine sees
  English. `TranslationsTest` checks that every language has every string
  with the same arguments as English.
- Exit: an in-app language choice (`AppCompatDelegate` locales or Android
  13's per-app language), and one source for both UIs' texts.

### Android: mpv's quality rendering shows a black picture
- Where: [mod.rs](../crates/cineo-player/src/embedded/mod.rs) (`SURFACE_OPTIONS`)
- Gap: on a Redmi 9 (Mali-G52, GLES 3.2) mpv's default `vo=gpu` rendering
  draws nothing; only turning off dithering, linear-light and correct
  downscaling and the non-bilinear scalers together shows the picture
  (seen 2026-11-03). Which shader stage fails is UNKNOWN.
- Why: not investigated further.
- Instead: surface playback uses `profile=fast` (bilinear, no dithering).
- Exit: find the failing stage on Mali, or test `vo=gpu-next`.

### Android: HEVC 10-bit decodes in software and draws in 8-bit
- Where: [mod.rs](../crates/cineo-player/src/embedded/mod.rs) (`SURFACE_OPTIONS`)
- Gap: on a Redmi 9 MediaTek's HEVC decoder fails on 10-bit video
  (`OMX.MTK.VIDEO.DECODER.HEVC` `ERROR(0x80001005)`), so mpv decodes it in
  software, which may not keep up with real 1080p content (only a test
  pattern was tried, 2026-11-05). The Mali GPU has no 16-bit UNORM
  textures, so mpv would draw 10-bit frames through the shader path that
  shows a black picture (entry above); `vf=format=fmt=yuv420p` converts
  software frames to 8-bit 4:2:0 on the CPU first, losing the extra
  precision and adding CPU work. Hardware frames pass through unchanged.
- Why: the picture was black for every 10-bit HEVC release (seen with
  Torrentio's x265 encodes).
- Exit: fix the shader path on Mali (entry above) and drop the conversion;
  try whether another MediaTek decoder setting accepts Main10.

### Android: torrents use mobile data and battery without limits
- Where: [shell.rs](../crates/cineo-android/src/shell.rs),
  [TorrentService.kt](../android/app/src/main/java/io/github/mdmrk/cineo/TorrentService.kt)
- Gap: a torrent streams over any network, mobile data included, and
  keeps DHT and uploads on with the desktop's defaults; how much battery
  and data this uses on a phone is UNKNOWN.
- Why: step 4 of ADR-0028 only brought torrents over.
- Instead: the P2P notice and the desktop's torrent settings (upload,
  limits, peers, DHT, storage, port; in Settings since 2026-11-05) are the
  controls on Android; none of them knows about mobile data.
- Exit: measure on a phone, then add a Wi-Fi-only option.

### Android: no subtitle style or delay in the player
- Where: [PlayerTracks.kt](../android/app/src/main/java/io/github/mdmrk/cineo/ui/PlayerTracks.kt)
- Gap: the desktop's Subtitles menu also changes the subtitle style and a
  delay for the playing video; Android's menu only picks a subtitle. The
  style is in Android's Settings since 2026-11-05 and applies from the
  next video.
- Why: the first Android subtitle slice brought over the menu and the
  preferred-language pick only.
- Instead: the style changes in Settings, before playing; no delay.
- Exit: the style controls and the delay buttons on Android, sending the
  same `PlayerCommand`s as the desktop.

### Android: audio focus is only taken and given up
- Where: [PlayerScreen.kt](../android/app/src/main/java/io/github/mdmrk/cineo/ui/PlayerScreen.kt) (`PlaybackFocus`)
- Gap: playback pauses when another app takes the audio focus, but does not
  resume when the focus comes back after a short loss, does not duck, and
  does not pause when headphones are unplugged (`ACTION_AUDIO_BECOMING_NOISY`).
  There is no media session, so no lock-screen or headset controls.
- Why: the first slice only stops Cineo and other apps playing at once.
- Exit: resume after `AUDIOFOCUS_GAIN` when the loss paused it, a
  becoming-noisy receiver, and a `MediaSession`.

### Android: no update check
- Where: [shell.rs](../crates/cineo-android/src/shell.rs) (`run_effect`)
- Gap: releases carry the APK (ADR-0029), but the app never says that a
  newer one exists.
- Why: the check was left out while Android had no release; the
  `prerelease` builds share a version with the release before them.
- Instead: the Android shell drops `Effect::CheckForUpdate`
  (`the_desktop_update_check_does_not_run`); users look at the Releases
  page and install the new APK over the old one.
- Exit: run the check on Android and open the release page, with a test.

### Android: prereleases share the release's versionCode
- Where: [build.gradle.kts](../android/app/build.gradle.kts) (`versionCode`)
- Gap: the `versionCode` comes from the workspace version (ADR-0030), so a
  prerelease APK has the same `versionCode` as the release before it.
  Android refuses only a lower `versionCode`, so it installs over it
  (INFERRED, not tried). Whether F-Droid's update checker can read a
  `versionCode` that Gradle computes instead of a literal is UNKNOWN.
- Why: F-Droid builds from a tag and cannot know CI's run number.
- Instead: prereleases install over each other with the same number.
- Exit: a `versionCode` that also counts prereleases and that F-Droid
  reads, with the check in `scripts/package.sh` kept.

### Android: the build is reproducible on one machine only
- Where: [build.gradle.kts](../android/app/build.gradle.kts) (`CargoNdk`),
  [build-libmpv-android.sh](../scripts/build-libmpv-android.sh)
- Gap: two release builds from different directories and `CARGO_HOME`s on
  the same machine give the same unsigned APK (VERIFIED 2026-11-05, SHA-256
  `2fe745f8…dbe0a7`). Whether F-Droid's Debian build server gives the same
  APK as CI's Ubuntu runner is UNKNOWN: Python, Perl and the host tools
  that generate libplacebo's and OpenSSL's code differ. Rust is the latest
  stable at release time (ADR-0025), so the recipe must name the version
  CI used, which `release-android.sh` prints. libmpv is built in the fixed
  `/tmp/cineo-libmpv-android`, so two libmpv builds cannot run at once on
  one machine, and its sources are downloaded again after a reboot.
- Why: F-Droid publishes our signed APK only if its build matches
  (ADR-0030); a fixed directory was the simplest way to make the paths
  that FFmpeg, mpv and OpenSSL record the same everywhere.
- Instead: `scripts/package.sh android` fails if a native library holds the
  build path, `HOME`, `CARGO_HOME`, `RUSTUP_HOME` or an OpenSSL build date;
  nothing compares two whole builds in CI.
- Exit: an F-Droid build of a tag that matches the release, then a CI job
  that builds twice and compares.

### Android: the release does not carry the libmpv source
- Where: [build-libmpv-android.sh](../scripts/build-libmpv-android.sh),
  [libmpv-android.env](../scripts/libmpv-android.env)
- Gap: the APK and the release carry no copy of the libmpv, FFmpeg and
  library sources; `libmpv.md` names the upstream URLs and hashes and
  the build script. Whether that meets LGPL-2.1 §4 and §6 is INFERRED,
  not legal advice.
- Why: the archive was 100 MB of a 155 MB APK, downloaded with every
  update (owner decision 2026-11-05, ADR-0029).
- Instead: if an upstream source is deleted, the build fails at its
  checksum and a user cannot get that source from us.
- Exit: none planned; move the pin to a source that is still published.

### Android: the libmpv build is not reproducible
- Where: [build-libmpv-android.sh](../scripts/build-libmpv-android.sh)
- Gap: rebuilding from the pinned sources gives working
  libraries whose hashes differ from the release's (seen 2026-11-04). The
  build directory is embedded in them, for example in FFmpeg's configure
  line and its source paths; whether anything else differs is UNKNOWN.
- Why: the LGPL asks for the source, not for identical binaries.
- Instead: `libmpv.md` names the hashes of the shipped files, and a
  rebuilt copy cannot be checked against them.
- Exit: `-ffile-prefix-map`, FFmpeg without its configure string, and a
  test that two builds in different directories match.

### Android: arm64 phones only
- Where: [build.gradle.kts](../android/app/build.gradle.kts)
  (`abiFilters`), [build-libmpv-android.sh](../scripts/build-libmpv-android.sh)
- Gap: the APK holds only `arm64-v8a` libraries, so 32-bit ARM and
  x86_64 devices (most emulators) cannot install it. The UI is built for
  touch: there is no Android TV launcher entry and no D-pad navigation.
- Why: ADR-0029. Each ABI doubles the libmpv build, and there is no other
  device to test on.
- Instead: Android TV boxes and older phones are unsupported.
- Exit: an Android TV slice (leanback launcher, focus handling), and
  more ABIs if someone can test them.

### Android: only some settings
- Where: [SettingsScreen.kt](../android/app/src/main/java/io/github/mdmrk/cineo/ui/SettingsScreen.kt),
  [shell.rs](../crates/cineo-android/src/shell.rs) (`SettingChange`)
- Gap: Android shows hardware decoding, seek step, watched-at, audio and
  subtitle language, local network addresses, clear history and reset.
  Subtitle style, audio output, volume, binge watching and the torrent
  settings are not shown.
- Why: the matching features (subtitle picking, next episode, torrents)
  are not on Android yet.
- Instead: those settings keep their saved values, and reset still
  resets them.
- Exit: add each one with its feature.

### Android: an addon subtitle is saved while the player lock is held
- Where: [shell.rs](../crates/cineo-android/src/shell.rs) (`subtitle_fetched`)
- Gap: the downloaded subtitle file is written to disk and handed to mpv
  while the shell holds the lock that every player call takes. The
  250 ms `player_status` poll runs off the UI thread, but pause, seek and
  track picks are called from the UI thread.
- Why: subtitle files are a few kilobytes, so the write is short
  (INFERRED); moving it out needs `SubtitleFiles` (shared with the
  desktop) to reserve a path, write unlocked and record it afterwards,
  and to cope with playback changing in between.
- Instead: a tap on a player control can wait for that write once per
  picked addon subtitle.
- Exit: write the file outside the lock.
