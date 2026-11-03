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
- `cineo-net` fetches addon manifests over HTTPS on Android, with
  certificates checked by the platform verifier (VERIFIED: spike step 3,
  2026-11-03, same phone as step 2). A throwaway `cdylib`, built against
  the workspace's `Cargo.lock` with `jni` 0.22.4, called
  `rustls_platform_verifier::android::init_with_env` with the application
  `Context` and then `AddonClient::fetch_manifest` with `NetPolicy::default()`
  from a Kotlin thread. The app pulled in the verifier's Kotlin component as
  `org.rustls:rustls-platform-verifier:0.2.0` from the project's Maven
  repository. Cinemeta and OpenSubtitles v3 manifests loaded with no
  warnings. `expired`, `self-signed`, `wrong.host` and `untrusted-root`
  at badssl.com were all rejected.
- Without that initialisation, the first HTTPS request panics in Rust and
  the app crashes (VERIFIED: same spike). The verifier must be initialised
  before any `cineo-net` call.
- The spike's JNI functions needed `#[unsafe(no_mangle)]`, which the
  workspace's `unsafe_code = "forbid"` rejects (VERIFIED: rustc 1.99.1
  reports "usage of the unsafe `no_mangle` attribute"; the spike crate was
  outside the workspace). `jni`'s
  `EnvUnowned::with_env` kept every other call safe.
- `cineo-store` finds its directories with `etcetera`, which has no notion
  of an Android app's private directories (INFERRED).
- `cineo-stream` uses the librqbit fork (ADR-0016), tokio and hyper.
- `cineo-core`, `cineo-net`, `cineo-store` and `cineo-stream` build for
  `aarch64-linux-android`, and so do their test binaries and `cineo-cli`,
  with Rust 1.99.1, NDK r30 and cargo-ndk 4.1.2 at API level 24 (VERIFIED:
  spike step 1, 2026-11-03, no source changes). At cargo-ndk's default API
  level 21, `cineo-stream`'s test binaries fail to link: `getifaddrs`,
  `freeifaddrs` and `pwritev` are missing from Bionic before API 24. They
  come in through `nix` 0.30.1, used by the librqbit fork (VERIFIED:
  `cargo tree -i nix`). The binaries were built, not run on a device.
- `cineo-player` loads libmpv at runtime through `libloading` and renders
  through the render API's `opengl` backend (ADR-0014). `HWDEC` is chosen
  with `cfg(not(windows))`, so Android would get `vaapi,auto-safe`
  (`crates/cineo-player/src/embedded/mod.rs`). mpv's `opengl` render backend
  also accepts GLES contexts and mpv has a `mediacodec` hwdec (INFERRED, not
  tried here). No arm64 Android `libmpv.so` is built or pinned.
- libmpv plays video on an Android phone with hardware decoding (VERIFIED:
  spike step 2, 2026-11-03, a Redmi 9 on LineageOS, Android 13, MediaTek
  MT6768, Mali GPU). The throwaway Kotlin app used the
  `jarnedemeulemeester/libmpv-android` v1.0.0 AAR (mpv 0.41.0 and its JNI
  wrapper) and gave mpv the `Surface` of a `SurfaceView` with `vo=gpu`,
  `gpu-context=android`, `opengl-es=yes`, `hwdec=mediacodec`. It did not
  use the render API: that API is for drawing into a toolkit's GL context,
  and a Kotlin UI has none. A 1080p30 H.264 stream played in real time
  through `OMX.MTK.VIDEO.DECODER.AVC` at about 50–60 % of one core, against
  about 210–250 % with software decoding.
- On that phone, hardware decoding only starts with FFmpeg's NDK MediaCodec
  (`vd-lavc-o=ndk_codec=1`). Through FFmpeg's default JNI path the codec
  never starts (`MediaCodec 0x0 failed to start`), with `mediacodec` and
  `mediacodec-copy` alike, and mpv falls back to software. Why is UNKNOWN.
- Going to the background and back, and rotating, worked with no crash
  (VERIFIED: same spike). On `surfaceDestroyed` the app sets `vo=null` and
  detaches the surface, and playback continues with software decoding; on
  `surfaceCreated` it reattaches, sets `vo=gpu`, and mpv goes back to
  `mediacodec`.
- The AAR's FFmpeg is built with `--enable-gpl --enable-version3`, so it is
  GPLv3, and the AAR needs API level 26 (VERIFIED: its `libavcodec.so` and
  manifest).
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
If we port, we take option 3: a native Kotlin/Jetpack Compose UI over the
Rust crates (chosen by the owner on 2026-11-03). `cineo-desktop` stays the
desktop shell and is not built for Android.

Whether we port at all (option 3 against option 1) is not decided yet. A
spike first answers the UNKNOWNs above:
1. Cross-compile `cineo-core`, `cineo-net`, `cineo-store` and
   `cineo-stream` for `aarch64-linux-android` with `cargo-ndk`. Done: they
   build at API level 24 (Context).
2. In a throwaway Kotlin app, load an arm64 `libmpv.so`, render a URL
   with `hwdec=mediacodec`, and survive a background/foreground cycle.
   Done on one phone, into a `SurfaceView` rather than through the render
   API (Context).
3. Fetch an addon manifest over HTTPS through `cineo-net` with the
   platform verifier initialised. Done (Context).
4. Choose the Kotlin binding generator (UniFFI or another) after checking
   its current version and docs, and call one `cineo-core` function from
   Kotlin through it.

The spike's results update this ADR, and it is accepted or rejected,
before any Android code is merged.

## Consequences
If accepted:
- Cineo has two UIs, `cineo-desktop` in Rust and the Android app in
  Kotlin. A user-visible feature is not done on Android until the Kotlin
  UI has it, and `docs/COMPATIBILITY.md` tracks each platform separately.
- The app's minimum is Android 7.0 (API level 24) or higher, because
  `cineo-stream` needs symbols Bionic only has from API 24.
- Kotlin and Gradle join the toolchain. The Android app's dependencies
  need their own license and version review (`docs/DEVELOPMENT.md`,
  `docs/LEGAL.md`).
- Typed messages from `cineo-core` (ADR-0017) are worded in Kotlin on
  Android, so the wording exists twice.
- ADR-0022/0027's platform scope changes, and `AGENTS.md`,
  `docs/ARCHITECTURE.md`, `docs/COMPATIBILITY.md` and `docs/DEVELOPMENT.md`
  follow.
- A new `cineo-android` crate (a new platform IO boundary, ADR-0002) holds
  the JNI code, with `unsafe_code` allowed in that crate only, as the
  libmpv FFI is in `cineo-player` (ADR-0014).
- The Android app initialises `rustls-platform-verifier` once, at
  startup, before any `cineo-net` call, and ships its Kotlin component at
  the version `Cargo.lock` pins.
- `cineo-store` and `cineo-stream` take their directories from the shell
  instead of finding them themselves on Android.
- On Android, `cineo-player` hands mpv a `Surface` (`wid`, `vo=gpu`)
  instead of using the render API, sets `hwdec=mediacodec` with
  `ndk_codec=1`, and detaches and reattaches the surface as Android
  destroys and recreates it.
- We build, pin and ship our own LGPL arm64 `libmpv.so` (and its FFmpeg),
  which `docs/LEGAL.md` must cover. The spike's AAR cannot ship: its FFmpeg
  is GPLv3.
- CI gains an Android build; `cargo deny` gains the Android graph.

## Rejected alternatives
- Option 2, an eframe shell for Android: rejected by the owner on
  2026-11-03. A touch and D-pad UI needs a redesign anyway, so sharing
  `cineo-desktop` code saves less than it seems, and eframe's Android
  support is experimental while Compose is Android's own toolkit and
  covers Android TV.
- Using Stremio's streaming server or a Stremio Android client as the
  backend: ruled out by ADR-0010.
- Replacing mpv with ExoPlayer/Media3 on Android: not rejected yet, but it
  would lose parity with the desktop player (ASS styling, the same
  subtitle and audio options) and needs its own ADR if proposed.
