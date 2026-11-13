# 0029. Android releases

- Status: Accepted
- Date: 2026-11-04
- Amends: 0028 (the LGPL question it left UNKNOWN)

## Context
ADR-0028 ports Cineo to Android and builds an LGPL arm64 `libmpv.so` and
FFmpeg from pinned sources (`scripts/build-libmpv-android.sh`). It left one
question open before a release: how the LGPL's requirement that users can
replace the library applies inside a signed APK. Until now the APK was only
built as a debug build, locally and in CI, and no release carried it.

What we know (2026-11-04):
- mpv is built with `-Dgpl=false` and FFmpeg without `--enable-gpl`,
  `--enable-version3` or `--enable-nonfree`, so both are
  LGPL-2.1-or-later, and the build script stops otherwise (VERIFIED:
  `docs/LEGAL.md`, the script's checks).
- The app ships `libmpv.so` and FFmpeg's libraries as separate shared
  libraries in `lib/arm64-v8a/` of the APK. `cineo-player` loads
  `libmpv.so` at runtime with `libloading`; nothing from them is linked
  into `libcineo_android.so` (VERIFIED: ADR-0014, the Gradle build).
- LGPL-2.1 §6 lets a "work that uses the Library" be distributed on terms
  of our choice if, among other things, it uses "a suitable shared library
  mechanism" that "will operate properly with a modified version of the
  library, if the user installs one" (§6(b)). §4 lets the library itself
  be distributed in object code if its complete source comes with it, and
  "offering equivalent access to copy the source code from the same place
  satisfies the requirement" (VERIFIED: the LGPL-2.1 text in
  `crates/cineo-desktop/assets/licenses/LGPL-2.1.txt`).
- LGPL-2.1 has no counterpart to GPLv3/LGPLv3's "Installation
  Information": it does not require that a modified copy keeps our
  signature or installs over ours (INFERRED from the license text; not
  legal advice).
- An APK is a zip file. A user can replace the `.so` files in
  `lib/arm64-v8a/`, align it with `zipalign` and sign it with their own key
  with `apksigner`, and Android installs it once the copy signed with our
  key is uninstalled. VERIFIED on 2026-11-04, on a release APK signed with
  a throwaway key: the steps in `libmpv.md`, with Python's `zipfile`
  standing in for `zip`, give an APK that `apksigner verify` accepts with
  the user's certificate, that `zipalign -c -P 16` passes, and that
  carries the new `libmpv.so`. It was not installed on a device, so
  whether Android installs and runs it is INFERRED.
- The phone used in ADR-0028's spike is arm64-v8a. The app is built for
  `arm64-v8a` only (`abiFilters`), and its UI is for touch, not a D-pad
  (VERIFIED: `android/app/build.gradle.kts`).

## Options considered
1. **Publish the APK on the same GitHub release as Linux and Windows,
   signed with our own key.** One place for every platform; `SHA256SUMS`
   and the build attestations cover it as well. The libmpv source can sit
   next to the APK as a second file, or inside the APK.
2. **A store (Google Play, F-Droid).** Store review, a developer account
   or F-Droid's own build of every dependency, including libmpv.
3. **Do not release; users build from source.** No signing key to keep,
   but almost nobody can install it.

## Decision
- CI builds a release APK on every push to `main` and every tag, signed
  with our release key, and publishes it as `cineo-arm64-android.apk` in
  the same GitHub release (or `prerelease`) as the Linux and Windows
  files, with the same build attestations. The key is a repository secret;
  the build fails rather than publish an unsigned APK. Pull requests build
  an unsigned debug APK, as before.
- The `versionCode` is CI's run number, so every release and prerelease
  counts upward; the `versionName` is the workspace version.
- arm64-v8a only. Android TV is left for later.
- LGPL: the APK loads `libmpv.so` and FFmpeg's libraries as separate shared
  libraries (§6(b)). The APK itself carries their source, uncompressed,
  as `assets/sources/libmpv-android-sources.tar.xz`: the pinned source
  tarballs, the libplacebo tree at its pinned commit, the build script
  and its `.env`, enough to rebuild the libraries without downloading any
  source (§4: the object code is accompanied by its source). The release
  has the APK as its only Android file (chosen by the owner on
  2026-11-04). The APK also carries the license texts and `assets/licenses/libmpv.md`, which
  gives the versions, the hash of each `.so` and the steps to replace
  them, and the app shows them under Settings, About.
- A release build fails if libmpv is missing, and CI checks that the APK
  holds `libmpv.so`, the FFmpeg libraries, `libmpv.md` and the source
  archive the build wrote, that the `.so` files are the ones the hashes
  name, and that `apksigner verify` passes.
- R8 code shrinking stays off: JNA and the UniFFI bindings use
  reflection (turned on by the 2026-11-05 amendment below).
- **Amended 2026-11-05 (owner decision): the APK carries no source.** The
  source archive was 100 MB of the 155 MB APK. The APK and the release no
  longer carry it; `libmpv.md` names each pinned upstream source with its
  URL and SHA-256 (libplacebo by commit) and the build script in this
  repository at the release's tag, as the Windows release does for its
  DLL. CI no longer checks for the archive. Whether pointing to upstream
  sources meets LGPL-2.1 §4 and §6 is INFERRED, not legal advice
  (`docs/TECHNICAL_DEBT.md`). The same change turns R8 on, with keep
  rules for JNA, the UniFFI bindings and rustls-platform-verifier
  (`android/app/proguard-rules.pro`), and stores the native libraries
  compressed (`useLegacyPackaging`): Android extracts them on install, so
  they take more space on the phone than in the APK. A local release
  build went from 155 MB to 23 MB (VERIFIED 2026-11-05: the dex from
  23.4 MB to 2.8 MB, the native libraries from 45 MB stored to about
  21 MB compressed, the `.so` hashes unchanged). Signed with a debug key,
  it started on the Redmi 9 with no exception in its log; playback with
  this build was not tried.

## Consequences
- The release key must be kept and backed up by the owner. If it is lost,
  installed copies cannot be updated; users would have to uninstall and
  lose their library.
- Every release has one more file. The source archive makes the APK
  about 100 MB larger (about 155 MB instead of 55 MB, measured
  2026-11-04), and every user downloads it with every update. The
  `ci / build (android)` job
  builds libmpv (about 7 minutes) when its script or pins change. The
  build is cached by the hash of the script and `.env`. Amended
  2026-11-13: tag builds also restore that cache, which only runs on
  `main` save. `build (android)` took 742 to 1131 s on tags and 167 s on
  `main` (VERIFIED on the run history); how much of the tag time libmpv
  took is UNKNOWN, because the job logs were not read. The Rust build of a
  tag still starts without a cache.
- A user who replaces libmpv signs with their own key, so they must
  uninstall our copy first and cannot take updates from us.
- `docs/LEGAL.md`, `docs/DEVELOPMENT.md`, `docs/COMPATIBILITY.md`,
  `README.md` and `docs/TECHNICAL_DEBT.md` (arm64 only, no Android TV)
  follow.

## Rejected alternatives
- Option 2, a store: not now. Google Play needs a developer account and
  its review; F-Droid builds every dependency itself, libmpv included.
  Either can be added later without changing this decision.
- Option 3, source only: the owner wants an installable app.
- The source archive as a second release file next to the APK: the owner
  wants one Android file per release (2026-11-04). Linking only to the
  upstream download URLs was not chosen: LGPL-2.1, unlike version 3, has
  no clause that lets the source sit on someone else's server.
- Also shipping armeabi-v7a, x86_64 or a universal APK: each doubles the
  libmpv build and has no device to test it on.
