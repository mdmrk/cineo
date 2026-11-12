# 0035. macOS support

- Status: Accepted
- Date: 2026-11-12
- Supersedes: the macOS parts of 0022 and 0027

## Context
ADR-0022 dropped macOS on 2026-10-13 and ADR-0027 kept it out. On
2026-11-12 the owner asked for a macOS release, built and shipped without
a Mac to try it on, taking crmne/spotifast as the model: it uses the same
fastframe crates and egui, and ships a universal `.app` in a `.dmg`.

Research (2026-11-12):
- fastframe v0.17.0 already has macOS code in the crates Cineo uses:
  Now Playing through `souvlaki` (`fastframe-now-playing`), App Nap and
  idle sleep (`fastframe-power`), the single-instance socket
  (`fastframe-instance`), and `~/Library` folders through `directories`
  (`fastframe-dirs`) (VERIFIED: source at the tag).
- `keepawake` 0.6.1, already used on Windows, has a macOS backend over
  IOKit power assertions (VERIFIED: source).
- media-kit/libmpv-darwin-build `v0.7.3` ships
  `libmpv-libs_v0.7.3_macos-universal-video-default.tar.gz`, SHA-256
  `324d3269…0d28` (VERIFIED: downloaded and hashed): universal (arm64 and
  x86_64) dylibs of mpv 0.36.0 built with `-Dgpl=false`, and FFmpeg 6.0
  with `--enable-version3` and no `--enable-gpl` (VERIFIED: the
  configuration strings in the binaries). The repository's license for
  this variant is MIT; the libraries keep their own licenses. Each dylib
  names its dependencies as `@rpath/…` and carries only build-machine run
  paths (VERIFIED: `llvm-otool -L`/`-l`). mpv 0.36.0 has client API 2.x,
  which Cineo accepts (mpv 0.35 or newer).
- macOS hands a URL scheme's links to an app bundle as Apple Events, not on
  the command line (spotifast `src/mac_links.rs`). Taking them needs an
  Objective-C handler, which is `unsafe`, and `unsafe` is forbidden outside
  the player's FFI (ADR-0014, ADR-0027).
- Signing with a Developer ID and notarizing needs a paid Apple account;
  there is none. arm64 refuses to run unsigned code, so the bundle is
  signed ad hoc.

## Options considered
1. **A universal `.app` in a `.dmg`, with the media-kit libmpv in
   `Contents/Frameworks`, signed ad hoc, without link handling.** Uses
   only code that exists; no `unsafe` in the desktop shell.
2. **Same, plus an Apple Event handler for `stremio://` and `cineo://`.**
   Needs `unsafe` Objective-C calls in the desktop shell.
3. **Ask users to install mpv with Homebrew.** No LGPL binaries to ship,
   but Homebrew's mpv is a GPL build with many libraries, and users must
   install it first.

## Decision
Option 1.

- **libmpv.** `scripts/fetch-libmpv-macos.sh` downloads the pinned archive
  (`scripts/libmpv-macos.env`), checks its SHA-256 and the license texts
  listed in `scripts/macos/licenses.txt`. `scripts/package.sh macos` copies
  the dylibs to `Cineo.app/Contents/Frameworks`, adds an `@loader_path` run
  path to each, signs them ad hoc and checks that `libmpv.dylib` loads.
  `cineo-player` looks for `libmpv.2.dylib` and `libmpv.dylib` next to the
  executable, then in `../Frameworks`, then on the system path, then in
  `/opt/homebrew/lib` and `/usr/local/lib`. `hwdec` is `auto-safe`.
- **App.** `Cineo.app` with `scripts/macos/Info.plist`
  (`io.github.mdmrk.cineo`, macOS 11 or newer), executable `cineo`, a
  `lipo` of the `aarch64-apple-darwin` and `x86_64-apple-darwin` builds,
  and an `.icns` made from the brand icon. Signed ad hoc, not notarized.
  Released as `cineo-universal-macos.dmg`.
- **Links.** No `CFBundleURLTypes`; the "Open stremio:// and cineo://
  links" setting is hidden on macOS and `register` fails there.
- **Platform code.** The D-Bus screen-saver inhibitor, `xdg-mime`
  registration and the `vaapi` hwdec are `cfg(target_os = "linux")`. Keep
  awake uses `keepawake` on Windows and macOS. Media keys, single
  instance, folders and App Nap go through fastframe as on the other
  desktops.
- **CI** runs clippy and the tests on `macos-latest`, and builds the
  `.dmg` for releases. `cargo deny` checks both macOS graphs.

## Consequences
- macOS behavior is UNKNOWN until someone runs it; COMPATIBILITY.md marks
  it Untested.
- Gatekeeper blocks the first open of a downloaded ad-hoc app. The README
  says how to allow it.
- macOS ships LGPL binaries from a third party: the bundle carries their
  notices in `Contents/Resources/licenses`, and LEGAL.md and SECURITY.md
  say where they come from.
- macOS gets an older libmpv (0.36.0) than Windows and Linux.
- `keepawake` now builds on macOS too; `zbus` is Linux-only.

## Rejected alternatives
- Option 2: it puts `unsafe` in the desktop shell, which ADR-0027 rejected
  for the same reason. Links can follow once fastframe (or another safe
  crate) delivers URL Apple Events.
- Option 3: a GPL libmpv and a manual install step, for a platform nobody
  can test.
