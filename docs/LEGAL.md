# Legal and provenance

**This is not legal advice.** It records how the project manages licensing
and provenance risks. Where a question matters, consult a qualified lawyer.
Statements below reflect the maintainers' understanding as of 2026-10-06.

## Our code
- Cineo's own code is licensed under the **MIT license** (`LICENSE`).
- Contributions are accepted under the same terms (inbound = outbound;
  CONTRIBUTING.md).
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
  (`deny.toml`). Extending the license allowlist requires a note here.
- Copyleft libraries (for example LGPL libmpv) are **not linked**. mpv runs as
  a separate program (ADR-0004). Bundling an mpv binary in installers (M7)
  must comply with mpv's license (GPL-2.0+ or LGPL-2.1+ depending on the
  build). This needs a review before the first installer.

## Trademarks and branding
- "Stremio" is a name of its respective owner. Cineo is **not affiliated
  with, endorsed by, or sponsored by** Stremio or Smart Code OOD.
- We use the name only descriptively, to state protocol compatibility
  ("works with Stremio-protocol addons"). It is never used as our product
  name, logo or domain, and never implied as an endorsement.
- No Stremio logos, icons, colors, fonts, screenshots or other assets are
  used.
- "Cineo" is a working name. A trademark search has **not** been done (open
  question).
- Handling the `stremio://` URL scheme could confuse users or conflict with
  the official app. It is undecided (GOALS.md).

## User-provided media and external addons
- Cineo ships no content and no content addons. Users choose which addons to
  install. Addons are operated by third parties who are responsible for
  their content.
- Users are responsible for complying with the laws that apply to them and
  to the content they access.
- Cineo will not ship, recommend or default-install addons that provide
  infringing streams. Default or suggested addons, if any, will be
  metadata-only and are reviewed per release.

## Metadata providers
Metadata (titles, posters, descriptions) comes from addons, which source it
from their providers (for example IMDb-derived data via Cinemeta). Cineo
displays it and caches it locally. It does not redistribute it. Any future
caching or export feature must be reviewed against provider terms.

## Adapted code

| File(s) | Source | License | Notes |
|---------|--------|---------|-------|
| — | — | — | None so far |

## Fixtures
Test fixtures are written by contributors. See
`tests/fixtures/addons/README.md` for the provenance rules.
