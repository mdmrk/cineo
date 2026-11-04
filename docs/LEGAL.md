# Legal and provenance

**This is not legal advice.** It records how the project manages licensing
and provenance risks. Where a question matters, consult a qualified lawyer.
Statements below reflect the maintainers' understanding as of 2026-10-06.

## Our code
- Cineo's own code is licensed under the **MIT license** (`LICENSE`).
- Contributions are accepted under the same terms (inbound = outbound;
  docs/DEVELOPMENT.md).
- Cineo is an **independent implementation**. No code is copied from Stremio
  repositories. If code is ever adapted from an open-source project, record it
  under *Adapted code* below **before** merging.

## Public protocol compatibility
- Cineo implements the client side of the addon protocol described in the
  public, MIT-licensed `stremio-addon-sdk` documentation.
- We studied the behavior of the MIT-licensed `stremio-core` and reimplemented
  it. Each such behavior is labelled VERIFIED-REF in
  [ADDON_PROTOCOL.md](ADDON_PROTOCOL.md).
- Implementing a documented protocol for interoperability is a common
  practice. We avoid private, undocumented APIs (ADR-0007).

## Reference projects and their licenses (studied, not copied)

| Project | License (as found) | How we use it |
|---------|--------------------|---------------|
| Stremio/stremio-addon-sdk (docs) | MIT | Protocol specification |
| Stremio/stremio-core | MIT | Behavior reference; architectural ideas |
| Stremio/stremio-web | GPL-2.0 | Architectural ideas only; **no code, assets or styles** |
| Stremio/stremio-shell-ng | **No license file found** (all rights reserved by default) | Architectural ideas only; **no code** |
| boykopovar/AnyPS5 | GPL-2.0 | Repository/process ideas only |

## Third-party crates
- Dependencies are permissively licensed and MIT-compatible (Apache-2.0
  dependencies are fine to use; their notices must ship with binaries), and checked by `cargo deny`
  (`.config/deny.toml`). Extending the license allowlist requires a note here.
  Current allowlist (2026-10-06): MIT, MIT-0, Apache-2.0 (incl. LLVM
  exception), BSD-2/3-Clause, ISC, Zlib, 0BSD, Unicode-3.0, Unlicense,
  CC0-1.0, CDLA-Permissive-2.0 (Mozilla CA bundle data via rustls).
  `cargo deny` checks the `x86_64-unknown-linux-gnu`,
  `x86_64-pc-windows-msvc` and `aarch64-linux-android` dependency graphs.
- One crate-scoped exception (2026-10-06): `option-ext` 0.2.0 is
  MPL-2.0. It reaches the tree through `librqbit-core` → `directories` →
  `dirs-sys` (ADR-0012) and cannot be removed without forking
  librqbit. MPL-2.0 is copyleft per file: we use it unmodified, so the
  obligation is to ship its notice and point to its source. The exception
  in `.config/deny.toml` covers only this crate; MPL-2.0 is not on the general
  allowlist.
- Crate-scoped exceptions for UniFFI (2026-11-03, ADR-0028): `uniffi`,
  `uniffi_bindgen`, `uniffi_core`, `uniffi_internal_macros`,
  `uniffi_macros`, `uniffi_meta`, `uniffi_pipeline` and `uniffi_udl`
  0.32.2 are MPL-2.0. `cineo-android` uses them, unmodified, to bind Rust
  to Kotlin; `uniffi_bindgen` and `uniffi_udl` only run at build time.
  The same MPL-2.0 terms apply as for `option-ext`: ship the notice and
  point to the source. The Kotlin that `uniffi-bindgen` writes is
  generated from our crate and is built into the app.

## Android app dependencies
The Android app (`android/`, ADR-0028) also uses Gradle dependencies,
which `cargo deny` does not see (VERIFIED: their POMs or crate manifests,
2026-11-03):
- AndroidX Compose (BOM 2026.09.00), `activity-compose` 1.13.0 and
  `lifecycle-runtime-compose` 2.11.0: Apache-2.0.
- `kotlinx-coroutines-android` 1.11.0: Apache-2.0.
- JNA 5.19.1, needed by the UniFFI Kotlin code: Apache-2.0 or
  LGPL-2.1-or-later, used under Apache-2.0.
- `org.rustls:rustls-platform-verifier` 0.2.0, the Kotlin half of
  `rustls-platform-verifier`: its crate `rustls-platform-verifier-android`
  0.2.0 is MIT or Apache-2.0; the Maven artifact is built from the same
  repository and its POM names no license (INFERRED: same license).
- `android_logger` 0.15.1 and `log` 0.4.34 (MIT or Apache-2.0) send Rust logs to logcat on Android.
- The Android app packages the libmpv build below when it exists locally; no release ships it yet. `scripts/build-libmpv-android.sh`
  builds the one it will ship (2026-11-03, ADR-0028), from the sources
  pinned in `scripts/libmpv-android.env`: mpv 0.41.0 with `-Dgpl=false`
  and FFmpeg 8.1.3 without `--enable-gpl`, `--enable-version3` or
  `--enable-nonfree`, so both are LGPL-2.1-or-later (VERIFIED:
  `libavutil` reports "LGPL version 2.1 or later" and mpv's `HAVE_GPL` is
  0). FFmpeg uses OpenSSL 3.5.9 (Apache-2.0) for HTTPS: FFmpeg's
  `configure` puts mbedTLS, not OpenSSL, on its version3 list. It also
  links dav1d 1.5.4 (BSD-2-Clause); mpv links libass 0.17.5 (ISC),
  freetype 2.15.0 (FTL), fribidi 1.0.18 (LGPL-2.1-or-later), harfbuzz
  15.0.0 (MIT), libplacebo 7.387.0 (LGPL-2.1-or-later) and the NDK's
  static libc++ (Apache-2.0 with LLVM exception). The archive carries
  each license text and the source list. The OpenSSL, dav1d, libass,
  fribidi and harfbuzz hashes match the ones those projects publish;
  FFmpeg, mpv and freetype publish none, so their hashes were taken from
  the first download.
- SQLite is compiled into the binary (`rusqlite` with `bundled`). SQLite
  is in the public domain; no notice is required.
- libmpv (LGPL-2.1+ or GPL-2.0+, depending on the build) is **not linked**.
  Cineo loads it at runtime with `libloading` (ADR-0014). Our bindings are
  written from mpv's ISC-licensed headers (`client.h`, `render.h`,
  `render_gl.h`, copyright the mpv developers); no mpv code is copied. We do
  not use the LGPL `libmpv2` crates. On Linux, Cineo uses the system's
  libmpv and ships none.
- The Windows release (`bundled-libmpv`, ADR-0027) embeds an unmodified
  LGPL build of `libmpv-2.dll` (mpv and FFmpeg, LGPL-2.1-or-later) from
  zhongfly/mpv-winbuild, pinned in `scripts/libmpv-windows.env`. It is
  written to disk and loaded as a separate shared library, and a user's own
  `libmpv-2.dll` next to the executable replaces it (LGPL-2.1 §6).
  `cineo-licenses.zip` carries `LGPL-2.1.txt` and `libmpv.md` with the
  binary's hash, the mpv commit and the build sources. Releases do not
  carry the archive itself; `libmpv.md` links to it upstream.

## Bundled assets
Fonts embedded in `cineo-desktop` (`crates/cineo-desktop/assets/fonts/`),
unmodified and not sold on their own, as the OFL requires. Each license
text ships next to its files and must accompany binary distributions: in
`cineo-licenses.zip` next to the executable in every release, which
`install.sh` copies to `$XDG_DATA_HOME/doc/cineo`.
Added 2026-10-07.

Cineo's logo, app icon and loading animation
(`crates/cineo-desktop/assets/brand/`) are the project's own artwork,
supplied by the owner on 2026-10-07 (logo updated and animation added
2026-10-08) and covered by the repository license.
The Android launcher icon (`android/app/src/main/res/mipmap-*/`) is the
same logo, resized onto the icon's background color (2026-11-04). The
Android app also carries the logo and the loading animation layers,
unmodified, in `android/app/src/main/res/drawable-nodpi/` (2026-11-04).

| Font | Files | Source | License |
|------|-------|--------|---------|
| Inter 4.1 (The Inter Project Authors) | `Inter-Regular.ttf`, `Inter-SemiBold.ttf` | <https://github.com/rsms/inter/releases/tag/v4.1> | SIL OFL 1.1 (`Inter-OFL.txt`) |
| Tabler Icons (Paweł Kuna), via the `iconflow` crate | `tabler-regular.ttf`, `tabler-filled.ttf` (inside the crate) | <https://github.com/tabler/tabler-icons> | MIT (notice in iconflow's `THIRD_PARTY_LICENSES_FONTS.md`) |
| DM Serif Display (Colophon Foundry; Adobe Source heritage) | `DMSerifDisplay-Regular.ttf` | <https://github.com/google/fonts/tree/main/ofl/dmserifdisplay> | SIL OFL 1.1 (`DMSerifDisplay-OFL.txt`) |
| egui's default fonts, via the `epaint_default_fonts` crate (fallbacks behind ours) | Ubuntu-Light, Hack, Noto Emoji, emoji-icon-font (inside the crate) | <https://github.com/emilk/egui/tree/master/crates/epaint_default_fonts> | Ubuntu Font Licence 1.0, SIL OFL 1.1, MIT (texts inside the crate); crate-scoped exception in `.config/deny.toml` (2026-10-07) |

The Android app embeds the same Inter and Tabler font files, unmodified,
in `android/app/src/main/res/font/`, and carries their license texts in
`android/app/src/main/assets/licenses/` (`Inter-OFL.txt`, and
`Tabler-MIT.txt` copied from iconflow's `THIRD_PARTY_LICENSES_FONTS.md`).
Added 2026-11-04.

## Trademarks and branding
- "Stremio" is a name of its respective owner. Cineo is **not affiliated
  with, endorsed by, or sponsored by** Stremio or Smart Code OOD.
- We use the name only descriptively, to state protocol compatibility
  ("works with Stremio-protocol addons"). It is never used as our product
  name, logo or domain, and never implied as an endorsement.
- No Stremio logos, icons, colors, fonts, screenshots or other assets are
  used.
- "Cineo" is the project's name (confirmed 2026-10-06). A trademark search
  has **not** been done.
- Cineo handles the `stremio://` URL scheme for interoperability with
  addon install links and page links that addon sites publish. It does not
  present itself as Stremio. Registration is opt-in from Settings and makes
  Cineo the `xdg-mime` default for both schemes (or the `HKCU` handler on
  Windows); Cineo never takes over the
  scheme without that click.

## User-provided media and external addons
- Cineo ships no content and no content addons. Users choose which addons to
  install. Addons are operated by third parties who are responsible for
  their content.
- Users are responsible for complying with the laws that apply to them and
  to the content they access.
- Cineo will not ship, recommend or default-install addons that provide
  infringing streams. Default or suggested addons, if any, will be
  metadata-only and are reviewed per release.

## Peer-to-peer streaming (ADR-0010)
- Torrent sources make the user's device join BitTorrent swarms. Their IP
  address becomes visible to peers, and they upload data while streaming.
  The legality of the content is the user's and the addon's responsibility.
- Cineo must disclose this before the first P2P stream and offer a setting
  that disables P2P sources entirely. Addons flagged `behaviorHints.p2p` are
  labelled at install time.

## Metadata providers
Metadata (titles, posters, descriptions) comes from addons, which source it
from their providers (for example IMDb-derived data via Cinemeta). Cineo
displays it and keeps library entries locally. It does not redistribute
it.

## Adapted code

`librqbit` (Apache-2.0) is used from the owner's fork with our patch
(ADR-0016). That is a dependency, not code in this repository.

| File(s) | Source | License | Notes |
|---------|--------|---------|-------|

## Fixtures
Test fixtures are written by contributors. See
`tests/addons/README.md` for the provenance rules.
