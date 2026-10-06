# Goals

Status vocabulary used across the docs: **VERIFIED** (tested or observed),
**INFERRED** (reasoned, not tested), **UNKNOWN** (not established).

## What we are building

**Cineo is an independent, local-first desktop media center written in Rust.**
It discovers content through third-party **addons that speak the publicly
documented Stremio addon protocol** (HTTP transport). With it you can browse
addon catalogs, view details, pick a stream, play it in a real media player
(mpv), and keep a personal library with watch progress.

**Product goal: the same user-facing behavior as Stremio** on the desktop.
That covers addons, browsing, stream sources (including torrents), playback,
library and deep links. The exceptions are cloud sync (not planned for now)
and anything that would require Stremio's private services. Owner decision,
2026-10-06 (ADR-0010).

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
- Its UI. We match behavior, not look.

Cineo **does** handle `stremio://` deep links (addon install and page links,
as documented in the SDK's `deep-links.md`) as well as its own `cineo://`.

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
- Deep links: `stremio://` (addon install and page links) and `cineo://`.
  Every link needs user confirmation before it changes state.
- `ytId` (YouTube) stream sources.
- Response caching that honors `Cache-Control`.
- Per-addon trust for self-hosted (private-network) addons.

- **Local streaming engine (ADR-0010)**: torrent sources (`infoHash`,
  `fileIdx`, `sources`) first, then archive sources (`rarUrls`, `zipUrls`,
  `7zipUrls`, `tgzUrls`, `tarUrls`), then `nzbUrl`.

### v1.0

- Stremio-equivalent behavior for the supported surface, on Linux, Windows
  and macOS, including torrent streams.
- Compatibility verified against a published corpus of real addons.
- Data export and import; schema migrations with tests.
- Signed release artifacts.

### Future research (not committed)

- **Embedded video.** libmpv rendering inside the app window instead of a
  separate mpv window.
- Casting (Chromecast, DLNA), and Android, iOS, TV and web/WASM targets.
- Live TV EPG (`epgProvider`, scheduled videos). This is needed for full
  parity, so it is scheduled after v1.0.

### Explicitly out of scope

- Hosting, indexing or recommending content sources. Cineo is a client.
- Stremio's private APIs, streaming server, branding or assets.
- The legacy (`/stremio/v1`) and IPFS addon transports, even though the
  reference client still supports legacy (owner decision 2026-10-06).
- Analytics and telemetry.
- **Cloud or account sync, for now** (owner decision 2026-10-06). Cineo is
  local-only. Data export and import cover moving between machines.
  Revisit with a new ADR if this changes.

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
| Playback, tracks, subtitle rendering | | | ✔ | | ✔ | |
| Deep links, file associations | | | ✔ | | | ✔ |
| Casting | | | | | ✔ | ✔ |
| Torrent / archive / NZB sources | | | ✔ | | ✔ | local streaming engine (ADR-0010) |

## Success looks like

1. A user installs Cinemeta plus a stream addon and goes from Board to playing
   a movie in under a minute, with no crashes on malformed addon data.
2. Every row in [COMPATIBILITY.md](COMPATIBILITY.md) marked Supported has an
   automated test, and every Verified row has a dated manual or live check.
3. A new contributor, human or agent, can make a correct, reviewed change by
   reading `AGENTS.md` and the docs it links. They do not need tribal knowledge.
