# 0028. Android port

- Status: Accepted
- Date: 2026-11-03
- Supersedes: the Android parts of 0022

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
- UniFFI 0.32.2 binds the Rust crates to Kotlin (VERIFIED: spike step 4,
  2026-11-03, same phone). The spike `cdylib` exported, with
  `#[uniffi::export]`, a sync function over `cineo_core::addon::parse_manifest`
  and an async one (`async_runtime = "tokio"`) over
  `AddonClient::fetch_manifest`, returning a `uniffi::Record` and a
  `uniffi::Error` enum. `uniffi-bindgen generate --library` read the arm64
  `.so` on the Linux host and wrote the Kotlin (about 1.2k lines). Kotlin
  got a data class, a sealed exception class and a `suspend` function: a
  manifest parsed, a bad one threw `InvalidManifest` with `cineo-core`'s
  reason, and Cinemeta loaded through the coroutine (about 1.2 s on Wi-Fi)
  while `expired.badssl.com` threw `Fetch`.
- UniFFI's macros compile under `#![deny(unsafe_code)]` (VERIFIED: same
  spike). Only the hand-written JNI entry point for the TLS verifier needed
  `#[allow(unsafe_code)]`, because UniFFI cannot pass a JNI `Context`.
- UniFFI is MPL-2.0, which `.config/deny.toml` does not allow today. Its
  generated Kotlin needs JNA 5.19.1 (Apache-2.0 or LGPL-2.1-or-later) and
  kotlinx-coroutines 1.11.0 (Apache-2.0) at runtime (VERIFIED: crates.io
  and the Maven Central POMs). BoltFFI 0.32.1 (MIT) is the other generator
  found; it is much younger (about 124k downloads against UniFFI's 16M) and
  was not tried.
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
  manifest). Its mpv also lists `gpl` among its enabled features.
- Third-party LGPL builds of libmpv for Android exist, all one-person
  projects from 2026 with at most one GitHub star (VERIFIED: GitHub,
  2026-11-03): `purestream711/libmpv-android`, `flow-vid/flowvid-libmpv-android`,
  `SnowMediaEnt/libmpv-android-lgpl`, `labyrinth-apps/libmpv-builds` and
  `pa2x2/mpv-android`. `purestream711/libmpv-android` v1.0.1-lgpl is an
  LGPL rebuild of the spike's AAR, with the same `MPVLib` API: its FFmpeg
  is configured with `--enable-version3` and without `--enable-gpl`, so it
  is LGPLv3, and its mpv does not list `gpl` (VERIFIED: its `libavcodec.so`
  and `libmpv.so`, AAR SHA-256 `820449224af6…`). It was not run on a
  device. The Windows build we ship is LGPL-2.1-or-later (`docs/LEGAL.md`).
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
   binding generator (UniFFI, chosen in spike step 4). libmpv is
   bundled as an arm64 `libmpv.so` in the APK. The best fit for phones and
   Android TV, at the cost of a second UI that does not share code with
   `cineo-desktop`.

## Decision
We port Cineo to Android (accepted by the owner on 2026-11-03), as
option 3:
- The UI is native Kotlin with Jetpack Compose, in an `android/` Gradle
  project. `cineo-desktop` stays the desktop shell and is not built for
  Android. The application id is `app.cineo`.
- A new `cineo-android` crate is the Android shell's Rust half. It holds
  the core `State`, runs `update`, and executes the `Effect`s through
  `cineo-net`, `cineo-store` and `cineo-stream`, as `cineo-desktop` does.
  Kotlin sends `Action`s and renders the state snapshots it is given; it
  holds no business logic (ADR-0011).
- `cineo-android` talks to Kotlin through UniFFI. Its only hand-written
  JNI is the TLS verifier's initialisation, in one module where
  `unsafe_code` is allowed.
- The minimum is API level 26 (Android 8.0): above the 24 that
  `cineo-stream` needs, and the level the spike's libmpv AAR declares.
- We build libmpv and FFmpeg for Android ourselves, as LGPL builds
  (mpv `-Dgpl=false`, FFmpeg without `--enable-gpl`), from pinned
  sources, rather than depend on a third-party build (chosen by the owner
  on 2026-11-03).

The spike that supports this (all on 2026-11-03):
1. `cineo-core`, `cineo-net`, `cineo-store` and `cineo-stream` build for
   `aarch64-linux-android` at API level 24 (Context).
2. libmpv plays into a `SurfaceView` with `hwdec=mediacodec` and survives
   a background/foreground cycle, on one phone (Context).
3. `cineo-net` fetches manifests over HTTPS with the platform verifier
   (Context).
4. UniFFI calls `cineo-core` and `cineo-net` from Kotlin, sync and async
   (Context).

## Consequences
- Cineo has two UIs, `cineo-desktop` in Rust and the Android app in
  Kotlin. A user-visible feature is not done on Android until the Kotlin
  UI has it, and `docs/COMPATIBILITY.md` tracks each platform separately.
- Kotlin and Gradle join the toolchain. The Android app's dependencies
  need their own license and version review (`docs/DEVELOPMENT.md`,
  `docs/LEGAL.md`).
- Typed messages from `cineo-core` (ADR-0017) are worded in Kotlin on
  Android, so the wording exists twice.
- ADR-0022's platform scope changes, and `AGENTS.md`,
  `docs/ARCHITECTURE.md`, `docs/COMPATIBILITY.md` and `docs/DEVELOPMENT.md`
  follow. Android stays unsupported in `docs/COMPATIBILITY.md` until a
  feature works there with a named test.
- The effect-running logic of `cineo-desktop` gets a second implementation
  in `cineo-android`. Where the two would copy each other, the shared part
  moves to the IO crate or to `cineo-core` instead.
- `.config/deny.toml` gains an MPL-2.0 exception for the UniFFI crates,
  and `docs/LEGAL.md` covers UniFFI, JNA and kotlinx-coroutines.
- The Android app initialises `rustls-platform-verifier` once, at
  startup, before any `cineo-net` call, and ships its Kotlin component at
  the version `Cargo.lock` pins.
- `cineo-store` and `cineo-stream` take their directories from the shell
  instead of finding them themselves on Android.
- On Android, `cineo-player` hands mpv a `Surface` (`wid`, `vo=gpu`)
  instead of using the render API, sets `hwdec=mediacodec` with
  `ndk_codec=1`, and detaches and reattaches the surface as Android
  destroys and recreates it.
- We maintain a build of LGPL arm64 `libmpv.so` and FFmpeg from pinned
  sources, which `docs/LEGAL.md` must cover. The spike's AAR cannot ship:
  it is GPL. How the LGPL's requirement that users can replace the library
  applies inside a signed APK is UNKNOWN and must be settled before a
  release.
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
