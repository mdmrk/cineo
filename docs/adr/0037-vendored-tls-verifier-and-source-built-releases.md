# 0037. The TLS verifier's Kotlin source in the tree, and releases built from source

- Status: Accepted
- Date: 2026-11-16
- Supersedes: the Maven part of 0028 and the libmpv cache on tags in 0029

## Context
Cineo is being submitted to the main F-Droid repository. Checking the
recipe against fdroidserver 2.4.5 and its `buildserver` image on 2026-11-16
found three things.

- F-Droid's scanner rejects `android/settings.gradle.kts`: it declares a
  Maven repository on `raw.githubusercontent.com` for
  `org.rustls:rustls-platform-verifier:0.2.0`, a prebuilt AAR (VERIFIED:
  `fdroid build` log, "Found unknown maven repo"). The AAR holds a 6.8 KB
  `classes.jar`, a manifest and `network_security_config.xml` (VERIFIED:
  the downloaded file).
- The upstream repository has the source of that AAR. The AAR's
  `network_security_config.xml` is byte for byte the one in upstream commit
  `2885f2fff6` ("Bump android-release-support to 0.2.0"), and in
  `0e76b93651`, `7c3e664e02` and `f4d7c8947c` (VERIFIED: SHA-256 of both).
  The file changed in later commits, so the commit matters.
- The APK on GitHub for v1.9.4 does not match a from-source build. Only
  `libavformat.so` differs, by the order of four undefined symbols
  (`dlopen`, `dlclose`, `dlerror`, `dlsym`) in its dynamic symbol table
  (VERIFIED: `readelf --dyn-syms`). The other six libmpv libraries and the
  Rust library are identical. A from-source build gives the same
  `libavformat.so` (SHA-256 `33bba58e…`) on Debian 13 with 12 CPUs, Debian
  13 with 4 CPUs and Ubuntu 24.04 with 4 CPUs (VERIFIED), so the build is
  deterministic. In the v1.9.4 release run, `build (android)` skipped the
  step that installs ninja, which only runs on a cache miss, so libmpv came
  from the Actions cache that an earlier run on `main` saved (VERIFIED:
  GitHub API job steps). Which property of that earlier run caused the
  different order is UNKNOWN: its logs were not read.

## Options considered
1. Keep the AAR and ask F-Droid to allow the repository: not up to us, and
   the scanner has no allow-list for it.
2. Build the AAR from upstream inside the F-Droid recipe: a second Gradle
   build with its own wrapper and plugin set in every recipe.
3. Put the upstream module in `android/` as a Gradle subproject.
4. Leave the libmpv cache on tags and drop `Binaries:` from the recipe, so
   F-Droid signs its own build: no reproducibility check, and users of the
   GitHub APK cannot update to the F-Droid one.
5. Skip the libmpv cache on tags, as `rust-cache` is already skipped.

## Decision
- `android/rustls-platform-verifier` holds the three upstream files that the
  AAR is built from (`AndroidManifest.xml`, `CertificateVerifier.kt`,
  `network_security_config.xml`), unchanged, at commit `2885f2fff6`, with
  both license texts and an `UPSTREAM` file naming the crate version and the
  commit. It is an Android library module with `BuildConfig.TEST = false`
  and minSdk 26. Upstream's consumer ProGuard rules are not used: they
  contain `-dontobfuscate`, which the AAR never applied to the app.
- `android/settings.gradle.kts` declares no repository outside Google and
  Maven Central. The app depends on `:rustls-platform-verifier`.
- Gradle fails the build when the version on the first line of `UPSTREAM`
  differs from `rustls-platform-verifier-android` in `Cargo.lock`, so a
  crate bump cannot leave the Kotlin side behind.
- On tags, `build (android)` does not restore the libmpv cache, so a
  release builds libmpv from the pinned sources. Pushes to `main` still
  restore and save it.

## Consequences
The scanner finding for the Maven repository goes away and the build no
longer needs network access to GitHub for Kotlin classes. Updating the
verifier means copying the files from a new upstream commit and changing
`UPSTREAM`. A release takes the time of a libmpv build longer, about 7
minutes on a 12 core machine. The first release built this way is the first
one an F-Droid build can match; v1.9.4 cannot be matched. Docs that change:
`docs/DEVELOPMENT.md`, `docs/LEGAL.md`, `docs/TECHNICAL_DEBT.md`,
`docs/fdroid/io.github.mdmrk.cineo.yml`, ADR-0028's index row.

## Rejected alternatives
The AAR, for F-Droid's scanner. Building it in the recipe, for the second
Gradle build it adds to every recipe change. Dropping `Binaries:`, because
it gives up the one check that ties the F-Droid build to the released one.
