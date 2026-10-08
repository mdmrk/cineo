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
  CC0-1.0, CDLA-Permissive-2.0 (Mozilla CA bundle data via rustls),
  BSL-1.0 (added 2026-10-06 for `clipboard-win` and `error-code`, the
  Windows clipboard behind egui's copy and paste; permissive, and it
  requires no notice in binary distributions).
- One crate-scoped exception (2026-10-06): `option-ext` 0.2.0 is
  MPL-2.0. It reaches the tree through `librqbit-core` → `directories` →
  `dirs-sys` (M9, ADR-0012) and cannot be removed without forking
  librqbit. MPL-2.0 is copyleft per file: we use it unmodified, so the
  obligation is to ship its notice and point to its source. The exception
  in `.config/deny.toml` covers only this crate; MPL-2.0 is not on the general
  allowlist.
- SQLite is compiled into the binary (`rusqlite` with `bundled`). SQLite
  is in the public domain; no notice is required.
- libmpv (LGPL-2.1+ or GPL-2.0+, depending on the build) is **not linked**.
  Cineo loads it at runtime with `libloading` (ADR-0014). Our bindings are
  written from mpv's ISC-licensed headers (`client.h`, `render.h`,
  `render_gl.h`, copyright the mpv developers); no mpv code is copied. We do
  not use the LGPL `libmpv2` crates. Bundling libmpv in installers
  (M7) must comply with its license: ship it as a separate, replaceable
  shared library, with its notices and source offer. This needs
  a review before the first installer.

## Bundled assets
Fonts embedded in `cineo-desktop` (`crates/cineo-desktop/assets/fonts/`),
unmodified and not sold on their own, as the OFL requires. Each license
text ships next to its files and must accompany binary distributions.
Added 2026-10-07.

Cineo's logo, app icon and loading animation
(`crates/cineo-desktop/assets/brand/`) are the project's own artwork,
supplied by the owner on 2026-10-07 (logo updated and animation added
2026-10-08) and covered by the repository license.

| Font | Files | Source | License |
|------|-------|--------|---------|
| Inter 4.1 (The Inter Project Authors) | `Inter-Regular.ttf`, `Inter-SemiBold.ttf` | <https://github.com/rsms/inter/releases/tag/v4.1> | SIL OFL 1.1 (`Inter-OFL.txt`) |
| Tabler Icons (Paweł Kuna), via the `iconflow` crate | `tabler-regular.ttf`, `tabler-filled.ttf` (inside the crate) | <https://github.com/tabler/tabler-icons> | MIT (notice in iconflow's `THIRD_PARTY_LICENSES_FONTS.md`) |
| DM Serif Display (Colophon Foundry; Adobe Source heritage) | `DMSerifDisplay-Regular.ttf` | <https://github.com/google/fonts/tree/main/ofl/dmserifdisplay> | SIL OFL 1.1 (`DMSerifDisplay-OFL.txt`) |
| egui's default fonts, via the `epaint_default_fonts` crate (fallbacks behind ours) | Ubuntu-Light, Hack, Noto Emoji, emoji-icon-font (inside the crate) | <https://github.com/emilk/egui/tree/master/crates/epaint_default_fonts> | Ubuntu Font Licence 1.0, SIL OFL 1.1, MIT (texts inside the crate); crate-scoped exception in `.config/deny.toml` (2026-10-07) |

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
  present itself as Stremio. If the official app is also installed, the OS
  decides which handler runs (per-platform behavior is UNKNOWN). Cineo never
  silently takes over the scheme from another installed handler.

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
- Archive and NZB sources fetch from servers the addon names (for NZB, Usenet
  servers with user credentials). The same disclosure principle applies.

## Metadata providers
Metadata (titles, posters, descriptions) comes from addons, which source it
from their providers (for example IMDb-derived data via Cinemeta). Cineo
displays it and caches it locally. It does not redistribute it. Any future
caching or export feature must be reviewed against provider terms.

## Adapted code

`librqbit` (Apache-2.0) is used from the owner's fork with our patch
(ADR-0016). That is a dependency, not code in this repository.

| File(s) | Source | License | Notes |
|---------|--------|---------|-------|

## Fixtures
Test fixtures are written by contributors. See
`tests/addons/README.md` for the provenance rules.
