# 0028. Android port

- Status: Proposed
- Date: 2026-11-03

## Context
Cineo supports Linux x86_64 and Windows x86_64 (ADR-0022, ADR-0027).
Android is listed as unsupported in `docs/COMPATIBILITY.md`. On 2026-11-03
the owner asked how viable an Android port is.

What the workspace looks like today (VERIFIED: source, 2026-11-03):
- `cineo-core` (about 4.6k lines) does no IO and depends only on `url`,
  `serde_json`, `semver`, `thiserror` and `percent-encoding`.
- `cineo-net` uses reqwest 0.13.5 with `rustls`, which pulls in
  `rustls-platform-verifier` 0.7.1 (`cargo tree -i`). On Android that
  verifier must be initialised from JNI with an Android `Context`, and the
  app must ship its Kotlin component (`rustls-platform-verifier-android`)
  (VERIFIED: the crate's README).
- `cineo-store` finds its directories with `etcetera`, which has no notion
  of an Android app's private directories (INFERRED).
- `cineo-stream` uses the librqbit fork (ADR-0016), tokio and hyper. Whether
  the fork builds for `aarch64-linux-android` is UNKNOWN.
- `cineo-player` loads libmpv at runtime through `libloading` and renders
  through the render API's `opengl` backend (ADR-0014). `HWDEC` is chosen
  with `cfg(not(windows))`, so Android would get `vaapi,auto-safe`
  (`crates/cineo-player/src/embedded/mod.rs`). mpv's `opengl` render backend
  also accepts GLES contexts and mpv has a `mediacodec` hwdec (INFERRED, not
  tried here). No arm64 Android `libmpv.so` is built or pinned.
- `cineo-desktop` (about 9.6k lines) is an eframe/egui shell built for a
  mouse and a keyboard: hover menus (the subtitle style panel opens on
  hover, `docs/COMPATIBILITY.md`), keyboard shortcuts, a single-instance
  socket or named pipe (ADR-0018), `xdg-mime`/`reg.exe` scheme registration,
  MPRIS/SMTC media keys and a keep-awake thread (ADR-0027).
- eframe 0.37.0 has `android-native-activity` and `android-game-activity`
  features, and `android-activity` 0.6.1 is already in the lock file
  (VERIFIED: eframe's `Cargo.toml`, `Cargo.lock`). How well eframe handles
  surface loss, the soft keyboard and insets on Android is UNKNOWN.
- The workspace sets `unsafe_code = "forbid"`. JNI calls need `unsafe`
  (INFERRED).

Android also differs from the desktop in ways that touch our design
(INFERRED): the GL surface is destroyed when the app goes to the
background, so the mpv render context must be torn down and recreated;
the OS stops background work unless a foreground service holds it, which
matters for torrent downloads; deep links arrive as intents.

## Options considered
1. **Do not port.** No cost; Android stays unsupported.
2. **All Rust: an eframe shell for Android.** Reuses the most code,
   including parts of `cineo-desktop`. The UI still needs a touch and
   D-pad redesign, and depends on eframe's experimental Android support.
   Platform services (MediaSession, keep screen on, intents, the TLS
   verifier) still need JNI.
3. **Native Android UI (Kotlin/Jetpack Compose) over the Rust crates.**
   A new `cineo-android` crate exposes `cineo-core`, `cineo-net`,
   `cineo-store`, `cineo-stream` and `cineo-player` to Kotlin through a
   binding generator (for example UniFFI; not yet researched). libmpv is
   bundled as an arm64 `libmpv.so` in the APK. The best fit for phones and
   Android TV, at the cost of a second UI that does not share code with
   `cineo-desktop`.

## Decision
Not decided. Before choosing between 2 and 3, a spike answers the
UNKNOWNs above:
1. Cross-compile `cineo-core`, `cineo-net`, `cineo-store` and
   `cineo-stream` for `aarch64-linux-android` with `cargo-ndk`.
2. In a throwaway app, load an arm64 `libmpv.so`, render a URL through the
   render API on a GLES context with `hwdec=mediacodec`, and survive a
   background/foreground cycle.
3. Fetch an addon manifest over HTTPS through `cineo-net` with the
   platform verifier initialised.

The spike's results, and the choice of UI, update this ADR before any
Android code is merged.

## Consequences
If accepted, in either form:
- ADR-0022/0027's platform scope changes, and `AGENTS.md`,
  `docs/ARCHITECTURE.md`, `docs/COMPATIBILITY.md` and `docs/DEVELOPMENT.md`
  follow.
- A new `cineo-android` crate (a new platform IO boundary, ADR-0002) holds
  the JNI code, with `unsafe_code` allowed in that crate only, as the
  libmpv FFI is in `cineo-player` (ADR-0014).
- `cineo-store` and `cineo-stream` take their directories from the shell
  instead of finding them themselves on Android.
- `cineo-player` gets an Android `HWDEC` value and a way to recreate its
  render context.
- We build, pin and ship an LGPL arm64 `libmpv.so` (and its FFmpeg),
  which `docs/LEGAL.md` must cover.
- CI gains an Android build; `cargo deny` gains the Android graph.

## Rejected alternatives
- Using Stremio's streaming server or a Stremio Android client as the
  backend: ruled out by ADR-0010.
- Replacing mpv with ExoPlayer/Media3 on Android: not rejected yet, but it
  would lose parity with the desktop player (ASS styling, the same
  subtitle and audio options) and needs its own ADR if proposed.
