# Goals

Status vocabulary used across the docs: **VERIFIED** (tested or observed),
**INFERRED** (reasoned, not tested), **UNKNOWN** (not established).

## What we are building

**Cineo is an independent, local-first desktop media center written in Rust.**
It discovers content through third-party **addons that speak the publicly
documented Stremio addon protocol** (HTTP transport). With it you can browse
addon catalogs, view details, pick a stream, play it in a real media player
(mpv), and keep a personal library with watch progress.

Cineo is its own product. It has its own name, code and design, and it is not
affiliated with Stremio. "Stremio-like" describes the *functional model*, not
the brand or the code (see [LEGAL.md](LEGAL.md)).

## Who it is for

- Desktop users who want a fast, native media center driven by community addons.
- Self-hosters who run their own addons on their own network.
- Developers who want a well-tested, open Rust implementation of the addon
  client side of the protocol.

## What "Stremio-like" means, functionally

1. **Addons are the content layer.** The app ships no content and no content
   sources. Users install addons by URL.
2. **Aggregation.** Catalogs, metadata, streams and subtitles are requested
   from every installed addon that declares support. The results are merged.
3. **Discover → Detail → Streams → Play.** This is the core user journey.
4. **Library and continue-watching** are built from playback progress.

## What compatibility means

An addon that works with the reference client should work with Cineo for the
**supported protocol surface** listed in
[ADDON_PROTOCOL.md](ADDON_PROTOCOL.md):

- Cineo sends the same requests: same URL shape, same encoding, same filtering.
- Cineo interprets the responses the same way, including the reference
  client's tolerance of malformed data.

Every compatibility claim is tracked in [COMPATIBILITY.md](COMPATIBILITY.md)
with a status. A claim needs a test before it can be "Supported", and testing
against real addons before it can be "Verified".

Compatibility does **not** cover:
- Stremio's private account API (`api.strem.io`).
- Its streaming server.
- Its UI.
- Its deep-link scheme. Whether Cineo may handle `stremio://` links is an
  open question.

## Scope

### MVP (the first release worth using)

| Capability | Needs |
|-----------|-------|
| Install/remove addons by manifest URL; order them | local persistence |
| Board and Discover: browse catalogs, filters (`genre`), paging (`skip`), search | addons |
| Detail page: meta, and the episode list for series | addons |
| Streams aggregated across addons; direct `http(s)` URL streams playable | addons, player |
| Playback in mpv: play/pause/seek, audio and subtitle track choice | player (mpv) |
| Subtitles from stream objects and subtitle addons | addons, player |
| Library, watch progress, continue watching | local persistence, player events |
| Desktop GUI on Linux and Windows | platform shell |

### v0.x (incremental after the MVP)

- macOS builds; packaging (AppImage/Flatpak, MSI, DMG).
- Binge-watching: next episode, with `bingeGroup` stream matching.
- `addon_catalog` (discovering addons from addons), addon configuration pages.
- Cineo's own deep links (`cineo://`), and opening addon install links.
- Response caching that honors `Cache-Control`.
- Per-addon trust for self-hosted (private-network) addons.

### v1.0

- A stable, documented MVP feature set on Linux, Windows and macOS.
- Compatibility verified against a published corpus of real addons.
- Data export and import; schema migrations with tests.
- Signed release artifacts.

### Future research (not committed)

- **Account sync.** This would need our own backend or a self-hostable sync
  server. Stremio's account API is out of scope.
- **Torrent, archive and NZB stream sources.** These need a local streaming
  engine (for example a Rust BitTorrent library). That carries legal and
  safety implications.
- **Embedded video.** libmpv rendering inside the app window instead of a
  separate mpv window.
- Casting (Chromecast, DLNA), and Android, iOS, TV and web/WASM targets.
- Live TV EPG (`epgProvider`, scheduled videos).

### Explicitly out of scope

- Hosting, indexing or recommending content sources. Cineo is a client.
- Stremio's private APIs, streaming server, branding or assets.
- The legacy (`/stremio/v1`) and IPFS addon transports.
- Analytics and telemetry.

## Platforms

| Platform | Status |
|----------|--------|
| Linux x86_64 | Initial target (primary development platform) |
| Windows x86_64 | Initial target |
| macOS aarch64 | Best-effort until v1.0; built and tested in CI |
| Android, iOS, TV, Web | Postponed. The core is kept free of IO so it can be reused. |

## What depends on what

| Capability | Addons | Own backend | Fully local | Account sync | Media player | Platform integration |
|-----------|:--:|:--:|:--:|:--:|:--:|:--:|
| Catalogs, meta, streams, subtitles lists | ✔ | | | | | |
| Installed addons, library, progress | | | ✔ | | | |
| Multi-device library | | ✔ | | ✔ | | |
| Playback, tracks, subtitle rendering | | | ✔ | | ✔ | |
| Deep links, file associations | | | ✔ | | | ✔ |
| Casting | | | | | ✔ | ✔ |
| Torrent sources | | | ✔ | | | streaming engine |

## Success looks like

1. A user installs Cinemeta plus a stream addon and goes from Board to playing
   a movie in under a minute, with no crashes on malformed addon data.
2. Every row in [COMPATIBILITY.md](COMPATIBILITY.md) marked Supported has an
   automated test, and every Verified row has a dated manual or live check.
3. A new contributor, human or agent, can make a correct, reviewed change by
   reading `AGENTS.md` and the docs it links. They do not need tribal knowledge.

## The smallest meaningful slice (next)

The first vertical slice (Milestone 1 in [ROADMAP.md](ROADMAP.md)) is a
headless `cineo` CLI that:

1. Fetches and validates a manifest.
2. Checks that a catalog is declared and that the extra arguments are
   acceptable.
3. Fetches that catalog through the network safety policy and prints it.

"Done" for the slice means it is tested against fixtures and against a mock
addon in CI, and checked live against at least one public addon.
