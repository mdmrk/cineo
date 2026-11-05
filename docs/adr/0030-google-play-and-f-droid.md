# 0030. Google Play and F-Droid

- Status: Accepted
- Date: 2026-11-05
- Amends: 0029 (which rejected a store "not now")

## Context
ADR-0029 releases a signed `cineo-arm64-android.apk` on GitHub and left
Google Play and F-Droid for later. The owner now wants the app in both,
F-Droid first because it costs nothing (owner decision 2026-11-05); Play
needs a paid developer account.

What we know (2026-11-05):

Both stores:
- One signature across channels matters: Android installs an update only
  if it is signed with the same key, so a user who moves between GitHub,
  Play and F-Droid builds signed with different keys must uninstall and
  loses their library (VERIFIED: ADR-0029's consequences).
- Apps targeting Android 15 or later must support 16 KB memory pages.
  `libmpv.so` and the FFmpeg libraries have their LOAD segments aligned to
  `0x4000` (VERIFIED 2026-11-05 with `llvm-readelf -lW` on a local
  build). `libcineo_android.so` was not checked; NDK r30 links with 16 KB
  alignment by default (INFERRED).
- The `versionCode` is CI's run number (VERIFIED: `build.gradle.kts`,
  `ci.yml`). It is not in the source tree, so a store that builds from a
  tag cannot know it.
- The Addons tab lists Cinemeta's Official and Community addon catalogs
  with Install buttons (VERIFIED: `docs/COMPATIBILITY.md`). How either
  store treats a community catalog that lists torrent addons is UNKNOWN.

Google Play:
- Play accepts only Android App Bundles (AAB) for new apps, and new apps
  use Play App Signing: Google holds the app signing key and re-signs the
  APKs it builds from the bundle; uploads are signed with an upload key.
  An existing key can be handed to Play instead of a generated one
  (INFERRED from Google's documentation as remembered; to be checked in
  the Play Console when the app is created).
- From 2026-08-31, new apps and updates must target API 36. The app
  targets 36 (VERIFIED: `android/app/build.gradle.kts`;
  <https://developer.android.com/google/play/requirements/target-sdk>).
- A personal developer account created after 2023-11-13 must run a closed
  test with at least 12 opted-in testers for 14 days before it can apply
  for production; organisation accounts are exempt (VERIFIED: Play Console
  Help, <https://support.google.com/googleplay/android-developer/answer/14151465>).
- Android developer verification: from 2026-09-30 certified devices in
  Brazil, Indonesia, Singapore and Thailand install apps only from
  verified developers, globally in 2027, sideloaded APKs included
  (reported by the press; whether the 2026-09-30 step happened as planned
  is UNKNOWN). A Play Console account is a verified developer, so it also
  covers the GitHub APK (INFERRED).
- `TorrentService` is a foreground service of type `mediaPlayback`,
  started while a torrent is open and stopped when it closes (VERIFIED:
  `TorrentService.kt`, `MainActivity.kt`). Play asks for a declaration of
  each foreground service type, with a justification and a video
  (INFERRED from Play policy as remembered).

F-Droid:
- F-Droid builds every app itself from a tagged source on its Debian build
  server, from a recipe in `fdroiddata`. It signs with its own key, unless
  the build is reproducible: then it publishes the developer's signed APK
  once its own build matches it (VERIFIED: F-Droid FAQ for app developers,
  <https://f-droid.org/docs/FAQ_-_App_Developers/>).
- Prebuilt toolchains from rustup are acceptable; the Android SDK and NDK
  are tolerated (VERIFIED: <https://f-droid.org/docs/Inclusion_Policy>).
- `scripts/build-libmpv-android.sh` downloads its pinned sources and checks
  their SHA-256 (VERIFIED: ADR-0029). Whether F-Droid lets the build
  download them, or wants them as `srclibs`, is UNKNOWN.
- JNA comes from Maven Central as an AAR with its own prebuilt
  `libjnidispatch.so` (VERIFIED: `build.gradle.kts`). Whether F-Droid's
  scanner accepts it or wants JNA built from source is UNKNOWN.
- Anti-features: F-Droid labels apps that depend on a non-free network
  service (`NonFreeNet`) or promote non-free add-ons (`NonFreeAdd`).
  Which, if any, its reviewers give Cineo is UNKNOWN.
- Reproducing our build byte for byte needs the same toolchain versions
  and path-independent output from Cargo, the NDK and the libmpv build
  (INFERRED). It has never been tried.
- IzzyOnDroid is a separate F-Droid-format repository that ships the
  developer's own APK from GitHub releases, with no build of its own
  (INFERRED from its public documentation as remembered; its size limit
  and inclusion rules are to be checked).

## Options considered
Signing:
1. **One key everywhere.** Hand our release key to Play App Signing, with a
   separate upload key; make the F-Droid build reproducible so F-Droid
   ships our signature. Users move freely between channels. Costs a
   reproducible build and gives Google our key.
2. **One key per channel.** Let Google generate the Play key and let
   F-Droid sign with its own. Nothing to reproduce, but three channels
   that never update each other.

F-Droid route:
1. **Main F-Droid repository**, built by F-Droid from our tags.
2. **IzzyOnDroid first**, with our GitHub APK, then the main repository.

## Decision
- F-Droid first; Google Play later, under the Play part of this decision,
  once the owner opens a developer account.
- F-Droid route 2: IzzyOnDroid from the next release, the main repository
  once the build reproduces.
- One key everywhere (signing option 1): the F-Droid recipe names our
  release certificate as the only allowed signer, so F-Droid publishes
  only builds that reproduce ours. When Play is added, our release key
  goes to Play App Signing (exported with Google's PEPK tool), with a new
  upload key kept as its own repository secret.
- The `versionCode` comes from the source: Gradle computes
  `MAJOR * 10000 + MINOR * 100 + PATCH` from the workspace version (10600
  for 1.6.0, above the last run number, 58, read from the GitHub API on
  2026-11-05), so CI, F-Droid and later Play all get the same number for
  the same tag. The build fails if MINOR or PATCH reaches 100, and
  `scripts/package.sh android` checks the APK's `versionCode` and
  `versionName` against the workspace version.
- CI keeps publishing the APK on GitHub as in ADR-0029, and checks every
  `.so` in it for 16 KB LOAD alignment.
- Store text lives in the repository in the fastlane layout
  (`fastlane/metadata/android/<locale>/`), which F-Droid, IzzyOnDroid and
  Play's API all read. Listings and screenshots show no third-party
  posters or titles and describe Cineo as a player for addons the user
  installs.
- Play, when added: CI builds an AAB (`bundleRelease`) on tags, signed
  with the upload key, and uploads it to the internal track through a
  Play Developer API service account; promotion stays manual. A privacy
  policy is published at a public URL in this repository.
  `TorrentService` is declared as `mediaPlayback`: it runs only while a
  torrent is open for playback.
- Before the first review in either store, the owner decides whether
  store builds list the Community addon catalog.

## Consequences
- Our release key is also held by Google. Losing the upload key is
  recoverable through Play support; losing the release key still breaks
  updates on every channel.
- F-Droid needs no new secret. Play will add the upload keystore and its
  passwords, and the service account's JSON key (`docs/SECURITY.md`).
- Until the F-Droid build reproduces, the app is not in the main F-Droid
  repository. Reproducibility must then be kept on every release, or
  F-Droid stops publishing updates.
- A personal Play account needs 12 testers for 14 days before production,
  and costs a one-time registration fee.
- `docs/DEVELOPMENT.md`, `docs/COMPATIBILITY.md`, `docs/LEGAL.md`,
  `docs/SECURITY.md`, `docs/ARCHITECTURE.md` and `README.md` follow when
  this is accepted; the UNKNOWNs above go to `docs/TECHNICAL_DEBT.md`
  until they are answered.

## Rejected alternatives
- Signing option 2: three builds that cannot update each other; a user
  switching channels loses their library.
- F-Droid route 1 alone: the app stays out of every F-Droid client until a
  reproducible build works, which has never been tried.
- Play or F-Droid only, no GitHub APK: drops users of the other channel
  and ADR-0029's attested release.
