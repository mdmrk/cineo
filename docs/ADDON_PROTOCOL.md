# Addon protocol

This document defines the protocol surface Cineo implements, and how. It is
the reference for `cineo-core::addon` (from Milestone 1 on). Implementation
status per feature lives in [COMPATIBILITY.md](COMPATIBILITY.md).

## Sources and how claims are labelled

- **SDK docs**: public protocol documentation,
  <https://stremio.github.io/stremio-addon-sdk/protocol.html> and `/api/`
  (MIT). Read 2026-10-06.
- **Reference client**: `stremio-core` (MIT), `development` branch, read
  2026-10-06. We describe its *behavior* and implement it independently. We
  do not copy its code.
- Labels: **VERIFIED-DOC** (stated in the SDK docs), **VERIFIED-REF** (read in
  reference-client source), **INFERRED**, **UNKNOWN**.

Where the docs and the reference client disagree, **Cineo follows the
reference client**. Addons in the wild are tested against it, not against
the docs. Each disagreement is listed under [Doc vs reference
differences](#doc-vs-reference-differences).

## Transport

| Transport | URL form | Cineo |
|-----------|----------|-------|
| HTTP(S) | `https://host/[prefix/]manifest.json[?query]` | **Supported (planned M1)** |
| Legacy | `…/stremio/v1` (JSON-RPC to v1/v2 addons) | Rejected (ADR-0007) |
| IPFS/IPNS | `ipfs://…/manifest.json` | Rejected (ADR-0007) |

**Transport URL invariants** (Cineo's `TransportUrl`):
- The scheme is `http` or `https`, and a host is present.
- The path ends with `/manifest.json`.
- No userinfo. Credentials in URLs leak into logs and process lists.
- No fragment.
- Any query string is kept and carried over to every resource URL.
  VERIFIED-REF: the reference client does a string replace of
  `/manifest.json`, which keeps the query.

The transport URL is the **identity of an installed addon** and may contain
secrets (configured addons put API keys in the path). Treat it as sensitive
(see [SECURITY.md](SECURITY.md)).

## Requests

`{base}/{resource}/{type}/{id}.json` or
`{base}/{resource}/{type}/{id}/{extra}.json`, where `base` is the transport
URL minus `/manifest.json`. VERIFIED-DOC, VERIFIED-REF.

Encoding (VERIFIED-REF):
- `resource`, `type` and `id` are each percent-encoded with the
  **`encodeURIComponent` set**. Alphanumerics and `- _ . ! ~ * ' ( )` stay
  literal; everything else is UTF-8 percent-encoded. That includes `:` in
  episode ids (`tt0944947:1:2` becomes `tt0944947%3A1%3A2`) and `/`.
- `extra` is `name=value` pairs joined by `&`. Names and values are encoded
  with the same set. `=` and `&` stay literal as separators. Example from the
  docs: `search=game%20of%20thrones&skip=100`.
- Order of extra pairs is preserved. Repeated names are allowed: a
  multi-select extra up to `optionsLimit`.

Resources used: `catalog`, `meta`, `stream`, `subtitles`, `addon_catalog`.
Unknown resource names in a manifest are preserved, not rejected.

## Manifest

| Field | Docs | Reference client | Cineo |
|-------|------|------------------|-------|
| `id` | required | required string | required, non-empty |
| `version` | required semver | **strict semver** (`semver::Version`) | strict semver (error otherwise) |
| `name` | required | required | required |
| `description` | required | **optional** | optional |
| `types` | required | required | required array; invalid entries skipped with warning |
| `resources` | required | required | required array |
| `catalogs` | required | defaults to `[]` | defaults to `[]` |
| `idPrefixes` | optional | optional | optional |
| `logo`, `background` | optional | invalid → absent | invalid or non-`http(s)` → absent + warning |
| `addonCatalogs` | optional | optional | optional |
| `behaviorHints` | optional | `adult, p2p, configurable, configurationRequired, epgProvider` | first four; `epgProvider` ignored (EPG out of scope) |
| `config` | optional | — | ignored until configuration pages (v0.x) |
| `contactEmail` | optional | optional | optional |

### Catalog declarations

- `type` and `id` are required. A catalog missing either is skipped with a
  warning.
- `name` is optional (VERIFIED-REF; the docs say required).
- Duplicates by `(type, id)`: the **first wins**, the rest are dropped.
  VERIFIED-REF (`unique_by`); Cineo also emits a warning.
- **Extra, full form:** `extra: [{ name, isRequired?, options?, optionsLimit? }]`.
  `optionsLimit` defaults to 1. Duplicate names: first wins.
- **Extra, short (legacy) form:** used when `extra` is absent:
  `extraSupported: [names]`, `extraRequired: [names]`. **Only names in
  `extraSupported` exist.** A name only in `extraRequired` is dropped.
  VERIFIED-REF; Cineo warns.
- An extra named `skip` is always normalized to optional, single-valued and
  without options. VERIFIED-REF.
- Cineo addition: `optionsLimit` below 1 or not an integer is treated as 1,
  with a warning.

### Resource filtering

"Should addon A be asked for path P?" (VERIFIED-REF, `Manifest::is_resource_supported`):

- **`catalog` / `addon_catalog`:** a declared catalog must match `type` and
  `id`, and every extra name in P must be declared, and every required extra
  must be present. **The `resources` list is not consulted** for catalogs.
- **Other resources:** the resource must be declared (first declaration
  wins), and then:
  - **Short form** (`"meta"`): inherits manifest-level `types` and `idPrefixes`.
  - **Full form** (`{name, types, idPrefixes}`): inherits **nothing**. A
    missing `types` matches **no** type. A missing `idPrefixes` matches
    **every** id.
  - `idPrefixes` that is absent or **empty** matches every id. Otherwise the
    id must start with one of the prefixes.

Cineo additionally rejects requests that send more values for an extra than
its `optionsLimit`. Cineo never builds such requests itself, so this does not
change which addons are asked.

## Responses

| Resource | Shape | Rules |
|----------|-------|-------|
| catalog | `{ "metas": [MetaPreview] }` | Missing `metas` is an error; `"metas": null` is an empty list (VERIFIED-REF); invalid items are skipped individually (VERIFIED-REF `VecSkipError`) |
| meta | `{ "meta": Meta }` | Missing/null `meta` or a meta without valid `id`/`type` is an error; `videos` sorted by (season, episode); video `title` falls back to `name` (VERIFIED-REF alias), `overview` to `description`; invalid videos skipped |
| stream | `{ "streams": [Stream] }` | Source chosen by field presence in reference order (`url`, `ytId`, `infoHash`, `externalUrl`, archives, `nzbUrl`); `infoHash` must be 40 hex chars; `title` backs `description`; invalid items skipped |
| subtitles | `{ "subtitles": [Subtitle] }` | `url` must be http(s); missing `lang` → `und` |

### MetaPreview fields Cineo keeps (M1)

`id` (required, non-empty), `type` (required), `name` (absent or null becomes
`""`), `poster`, `posterShape` (`poster`|`square`|`landscape`, default
`poster`), `background`, `logo`, `description`, `releaseInfo` and `imdbRating`
(**string or number**, normalized to string), `genres`.

Ignored for now: `links`, `trailers`, `trailerStreams`, `director`, `cast`.
These are listed in COMPATIBILITY.md.

### Lenient parsing rules (ADR-0003)

- An empty string counts as absent.
- A wrong JSON type for an optional field is treated as absent, with a warning.
- Image URLs must be `http(s)`. Anything else (`file:`, `javascript:`, …) is
  dropped with a warning.
- A non-string element in a string array is skipped with a warning.
- Warnings carry a location (`catalogs[2].extra[0]`) and are logged at `warn`
  level by the IO layer.

### Streams (planned, M2)

A stream source is one of `url`, `ytId`, `infoHash`(+`fileIdx`), `externalUrl`,
`nzbUrl` or the archive sources (`rarUrls`, …) (VERIFIED-DOC). Cineo plays
**`url` with `http(s)`** first (M3). Other sources are parsed from M2 on and
shown as "not yet supported" until the local streaming engine supports them
(ADR-0010: torrents in M9, archives and NZB in M10). They are never fetched
before then.
`behaviorHints.proxyHeaders.request` become player HTTP headers only after
validation (no CR/LF; header names restricted to tokens).

Torrent streams (`infoHash`, M9; ADR-0012):
- The file to play is `fileIdx` if it names a file, else the file whose name
  equals `behaviorHints.filename` (case-insensitive, folders ignored), else
  the largest video file, else the largest file. The SDK documents only the
  "largest file" default; the rest is INFERRED
  (`choose_file` unit tests in `cineo-core`).
- `sources` entries `tracker:<url>` add `http(s)`/`udp` trackers. `dht:`
  entries add nothing, since DHT is always used. Other entries are ignored.
- Real addons may send no `sources` at all: Torrentio's streams had only
  `infoHash`, `fileIdx` and `behaviorHints.filename` (VERIFIED 2026-10-06).

Subtitles (M6):
- When a stream plays, every addon that supports `subtitles` for the item's
  type and the **video** id is asked for `subtitles/{type}/{videoId}`. The
  extras are `videoHash`, `videoSize` and `filename`, in that order, taken
  from the stream's `behaviorHints`; missing ones are left out (VERIFIED-REF,
  `models/player.rs` `subtitles_update`).
- The reference client also fills these extras from values its player
  reports and skips the request when it has none at all. Cineo's player
  reports none yet, so Cineo always asks, without extras if the stream has no
  hints (INFERRED to match the reference client, whose player always reports
  something; `subtitle_request_without_hints_has_no_extras`).
- The stream's own `subtitles` are listed first, then each addon's, in user
  order. Duplicate URLs within one list are dropped.

## Doc vs reference differences

| Topic | SDK docs | Reference client | Cineo follows |
|-------|----------|------------------|---------------|
| catalog `name` | required | optional | reference |
| manifest `description` | required | optional | reference |
| `extraRequired` not in `extraSupported` | unspecified | dropped | reference (+ warning) |
| Full-form resource without `types` | `types` required | matches nothing | reference (+ warning) |
| Duplicate catalogs | unspecified | first wins | reference (+ warning) |
| The minimal example in `protocol.md` | uses `/subtitle/` in one place | `/subtitles/` | `subtitles` |

## Deep links (planned, v0.x)

From the SDK's `deep-links.md` (VERIFIED-DOC):
- **Addon install:** `stremio://<host>/<path>/manifest.json`. This is the
  manifest URL with `https://` replaced by `stremio://`. Cineo maps it back
  to `https://` and always shows the install confirmation. Mapping to
  `http://` is never assumed.
- **Pages:** `stremio:///board`, `stremio:///discover[/{encodedAddonUrl}/{type}/{catalogId}?genre=…]`,
  `stremio:///library`, `stremio:///search?search=…`,
  `stremio:///detail/{type}/{id}[/{videoId}]?autoPlay=…`.
- Deep links are untrusted input. They are parsed into typed routes, unknown
  links are rejected with a message, and nothing is installed or played
  without user action. `autoPlay` is documented as Android-TV-only;
  Cineo's handling of it is UNKNOWN until M8.

## Caching (planned, v0.x)

The SDK sets `Cache-Control: max-age`, `stale-while-revalidate` and
`stale-if-error` on responses (VERIFIED-DOC). Whether the reference client
honors them in the app is **UNKNOWN**. Cineo plans an in-memory cache keyed by
the full request URL that honors `max-age` and `stale-if-error`, with an upper
bound. There is no caching before M2.

## Errors, timeouts and partial failure

- Per-request timeout of 20 s total and 10 s connect (initial values;
  [SECURITY.md](SECURITY.md)).
- Non-2xx status, invalid JSON, a missing top-level field, a policy block or
  an oversized body make **that addon's** result fail. In aggregations, other
  addons' results are still shown, and the failure is visible per addon.
- A request for a path the addon does not declare is never sent.

## Security considerations

See [SECURITY.md](SECURITY.md). In short:
- All addon data is untrusted.
- Requests go through the network policy (no private networks by default,
  redirects re-validated, size caps on decoded bytes).
- Addon-supplied URLs are never passed to the player without scheme checks.
- Full transport URLs are never logged.

## Adding protocol behavior

Steps:
1. Document the behavior here with a source label.
2. Add a fixture under `tests/addons/`.
3. Add a test that names the behavior.
4. Update COMPATIBILITY.md.
