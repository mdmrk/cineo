# Debugging

Failures must be observable without a debugger. This page says what each
subsystem logs and how to diagnose it. Sections marked (M_n_) describe tools
that arrive with that milestone.

## Logging

- `tracing` everywhere. Libraries never print. Only shells write to
  stdout/stderr.
- CLI (M1): `-v` gives debug logs for Cineo crates, `-vv` gives trace for
  everything. `RUST_LOG` overrides both, for example
  `RUST_LOG=cineo_net=trace`. Logs go to **stderr**, so stdout stays
  machine-readable.
- GUI (M5): a rotating log file in the platform log directory. The path is
  shown in an "About / Diagnostics" view.

### Span conventions

| Span | Fields | Emitted by |
|------|--------|-----------|
| `addon_request` | `req` (process-unique id), `addon` (origin only), `resource` (encoded resource path) | `cineo-net` |
| `playback` | `session`, `meta_id`, `video_id`, `source` (scheme + host) | player (M3) |
| `startup` | `phase` | shells |

**Never log** full transport URLs, query strings of addon URLs, stream URLs
with tokens, or headers from `proxyHeaders`. Log the origin and path instead.

## Diagnosing

### Addon failures

1. `cineo -v addon inspect <manifest-url>` (M1). This shows the parsed
   manifest, every **warning** (field location + reason), and the request
   span.
2. Read the error type. It is one of:
   - `blocked by network policy` (private address, redirect downgrade, too
     many redirects)
   - `timed out`
   - `HTTP <status>`
   - `exceeds the N-byte limit`
   - `invalid manifest: …` (the field is named)
3. If another client accepts the addon and Cineo does not, compare against
   [ADDON_PROTOCOL.md](ADDON_PROTOCOL.md). Then file an "Addon
   compatibility" issue with the inspect output.

### Catalog / metadata failures

- `cineo -v catalog <url> <type> <id> [--extra k=v]` (M1). It refuses
  undeclared catalogs and invalid extras **before** sending a request, and
  the message says which rule failed.
- Skipped items appear as `warn` lines with `metas[i]` locations.
- An empty result with no warnings means the addon returned
  `"metas": []` or `null`.

### Stream failures (M2/M3)

- `cineo streams <type> <video-id>` lists streams per addon, including the
  unsupported ones and why they are unsupported (torrent, archive, …).
- Check the stream source scheme. Only `http(s)` is playable.

### Playback failures (M3)

- `cineo play <url>` with `-v` logs every IPC command sent and every event
  received, as JSON.
- mpv's own log: the player passes `--log-file` into the Cineo log directory
  when debug logging is on.
- Common causes: an unsupported codec (check `hwdec` fallback in the mpv log),
  an HTTP 403 (check `proxyHeaders`), an expired stream URL.

### Subtitle failures (M6)

- Logged per subtitle: source addon, language, URL origin, load result from
  mpv `track-list`.

### Persistence problems (M4)

- `cineo doctor` prints the database path, schema version, row counts and an
  integrity-check result.
- Migrations log `from → to` at `info`. A failed migration leaves the old
  database untouched (backup first).

### UI / core synchronization (M5)

- Every action and every effect result goes through one dispatch function,
  which logs at `debug` with a sequence number. A UI showing stale data means
  a missing state emission. Check that the reducer test covers the action.

## Reproducible bug reports

A good report contains:
1. The exact command or UI steps.
2. `-v` logs, scrubbed of personal data.
3. The version (`cineo --version`) and OS.
4. For addon issues: a **public** manifest URL only.

The issue templates ask for these.
