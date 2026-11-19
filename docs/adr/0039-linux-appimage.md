# 0039. A Linux AppImage with its own LGPL libmpv

- Status: Accepted
- Date: 2026-11-19
- Amends: the Linux parts of 0014 and 0027 (Linux keeps the system libmpv)

## Context
Linux releases ship one executable, `cineo-x86_64-linux`, that loads the
system's `libmpv.so.2` (ADR-0014, ADR-0027). Users must install their
distribution's mpv first (README), `install.sh` warns when it is missing,
and the mpv they get differs per distribution. Windows, macOS and Android
already carry an LGPL libmpv of their own (ADR-0027, ADR-0035, ADR-0029).
The owner asked on 2026-11-18 for an AppImage with libmpv inside and
delta updates, after comparing with crmne/spotifast's releases.

Facts (2026-11-18):
- On Linux, `cineo-player` tries `libmpv.so.2` beside the executable
  before the system names (`embedded::ffi::candidates`), so a bundled copy
  needs no code change (VERIFIED: source).
- Distribution libmpv cannot be bundled: Debian and Ubuntu build FFmpeg
  with `--enable-gpl` (INFERRED from their packaging); shipping it would
  put GPL code in a release, which Cineo avoids on every platform.
- `scripts/build-libmpv-android.sh` already builds mpv 0.42.0, FFmpeg
  8.1.3, libplacebo, libass, HarfBuzz, FreeType, FriBidi, dav1d and
  OpenSSL from pinned, hashed sources and fails on GPL, version3 or
  nonfree configuration (VERIFIED: source).
- The desktop asks for system fonts by name (`sub-font` serif or
  sans-serif) and for `hwdec=vaapi,auto-safe` (VERIFIED: source), so libass
  needs fontconfig and mpv needs VA-API.
- The Linux build runs on `ubuntu-24.04` with glibc 2.39 (`fix(ci): build
  the Linux binary on Ubuntu 24.04`); the AppImage has the same floor.
- AppImage tooling with fixed releases: `AppImage/type2-runtime`
  `20251108` (`runtime-x86_64`, SHA-256 `2fca8b44…ec260d`), a static
  runtime that does not need libfuse2, and `AppImage/appimagetool` `1.9.1`
  (`appimagetool-x86_64.AppImage`, SHA-256 `ed4ce84f…384eb0`) (VERIFIED:
  GitHub releases, downloaded and hashed). Both projects also publish a
  moving `continuous` release, which cannot be pinned.
- AppImage update information `gh-releases-zsync|<owner>|<repo>|latest|<file>.zsync`
  lets AppImageUpdate, AppImageLauncher, AM and Gear Lever download only
  the changed blocks (crmne/spotifast does this; VERIFIED: its
  PACKAGING.md).
- Which libraries every desktop has differs; libva's compiled-in driver
  directory differs between Debian (`/usr/lib/x86_64-linux-gnu/dri`) and
  Arch or Fedora (`/usr/lib/dri`, `/usr/lib64/dri`) (INFERRED).
- libva 2.24.1 (`libva-2.24.1.tar.bz2`, SHA-256 `eec6050b…483d67`) splits
  its default driver path on `:` like `LIBVA_DRIVERS_PATH`, and looks in a
  driver for `__vaDriverInit_1_24` down to `__vaDriverInit_1_0` (VERIFIED:
  `va/va.c`). A host driver built against a newer libva exports only a
  newer name, so this libva would not load it.

## Options considered
1. **AppImage with libmpv built from source like Android's**, next to the
   plain executable. Same LGPL guarantees as the other platforms; a second
   from-source libmpv build to maintain and cache.
2. **AppImage with the distribution's libmpv and its dependencies**
   (linuxdeploy). Little build work; GPL FFmpeg in the release, and a few
   hundred shared libraries of one distribution.
3. **A third-party prebuilt Linux libmpv.** None published as a pinned,
   LGPL, self-contained library was found (2026-11-18).
4. **Flatpak instead.** Sandboxed and updated by the distribution's
   tooling, but a different packaging, review and runtime model; it can
   come later.

## Decision
Option 1.

- **libmpv.** `scripts/build-libmpv-linux.sh` with
  `scripts/libmpv-linux.env` builds the Android set natively on
  `ubuntu-24.04`, plus libva (a shared library only, so shipped beside
  libmpv, with the driver directories of Debian, Fedora and Arch in its
  search path), with the same GPL,
  version3 and nonfree checks. mpv is built with `-Dgpl=false`, VA-API
  over DRM, ALSA and PulseAudio output, and fontconfig for libass, and
  without X11, Wayland and EGL.
  Its output is cached like Android's.
- **Libraries.** The AppImage carries `libmpv.so.2`, FFmpeg's shared
  libraries, `libva.so.2` and `libva-drm.so.2`, found through
  `RUNPATH=$ORIGIN`, never `LD_LIBRARY_PATH`. The other libraries built
  here are linked statically and not exported (`--exclude-libs,ALL`).
  It takes from the host only libraries every desktop has and that must
  match the host: glibc, libgcc, libstdc++, zlib, libdrm, fontconfig,
  ALSA and PulseAudio. The build fails if a bundled library needs anything
  else.
- **Image.** `cineo-x86_64-linux.AppImage` from the pinned type2 runtime
  and appimagetool, both checked by SHA-256, with the `.desktop` entry,
  the icon, the licenses and a `libmpv.md` like the other platforms'.
  It embeds
  `gh-releases-zsync|mdmrk|cineo|latest|cineo-x86_64-linux.AppImage.zsync`,
  and `cineo-x86_64-linux.AppImage.zsync` ships beside it. Both are in
  `SHA256SUMS`, which is signed.
- **Checks in CI.** The AppImage starts (`--version`) in clean Ubuntu
  24.04, Debian 13 and Fedora containers, its bundled libraries resolve
  (`ldd` shows nothing missing), and its update information is read back
  and matches the `.zsync` file.
- **Unchanged.** `cineo-x86_64-linux` and `install.sh` stay as they are,
  with the system libmpv.

## Consequences
- Linux users can run Cineo without installing mpv, and update it with
  AppImage tools without downloading all of it.
- A second libmpv build to keep current with Android's pins; its cache
  must be pruned like the others.
- Release size grows by the AppImage (tens of MB, INFERRED) and its
  `.zsync` file.
- mpv's VA-API interop, which draws decoded frames without a copy, needs
  the window's X11 or Wayland display (`embedded::render`) and an mpv
  built with X11 or Wayland, which would add libX11, libXpresent, libXss
  and more to the host libraries (INFERRED). This build leaves them out,
  so the AppImage decodes in hardware at most through `vaapi-copy` over
  DRM, else in software (UNKNOWN which); recorded in
  `docs/TECHNICAL_DEBT.md`.
- When a host's VA-API driver is newer than the bundled libva, hardware
  decoding falls back to software (`auto-safe`) until libva is updated
  here; recorded in `docs/TECHNICAL_DEBT.md`.
- UNKNOWN until run on real desktops: hardware decoding through the
  bundled libva on Arch, Fedora and Debian; audio on PipeWire through
  libpulse; subtitles with each desktop's fonts.
- Docs: `docs/ARCHITECTURE.md`, `docs/COMPATIBILITY.md` (Linux row and a
  Details section), `docs/DEVELOPMENT.md` (release files, builds, caches,
  tools), `docs/LEGAL.md` (the bundled libraries), `README.md` (Install).

## Rejected alternatives
- **Distribution libmpv (2):** GPL FFmpeg in the release, against the LGPL
  rule every other platform follows.
- **Third-party prebuilt (3):** nothing pinned and LGPL to depend on.
- **Flatpak only (4):** not rejected for good; it does not replace a
  download that runs everywhere and updates in place.
