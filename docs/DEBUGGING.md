# Debugging

Failures must be observable without a debugger. This page says what each
subsystem logs and how to diagnose it.

## Logging

- `tracing` everywhere. Libraries never print. Only shells write to
  stdout/stderr.
- CLI: `-v` gives debug logs for Cineo crates, `-vv` gives trace for
  everything. `RUST_LOG` overrides both, for example
  `RUST_LOG=cineo_net=trace`. Logs go to **stderr**, so stdout stays
  machine-readable.
- GUI: `cineo-desktop` logs to stderr with the same `-v`/`-vv`/
  `RUST_LOG` rules. A log file and a diagnostics view are not implemented
  yet ([TECHNICAL_DEBT.md](dev/TECHNICAL_DEBT.md)).

### Span conventions

| Span | Fields | Emitted by |
|------|--------|-----------|
| `addon_request` | `req` (process-unique id), `addon` (origin only), `resource` (encoded resource path) | `cineo-net` |
| `playback` | `session`, `meta_id`, `video_id`, `source` (scheme + host) | player |
| `startup` | `phase` | shells |

**Never log** full transport URLs, query strings of addon URLs, stream URLs
with tokens, or headers from `proxyHeaders`. Log the origin and path instead.

## Diagnosing

### Addon failures

1. `cineo -v addon inspect <manifest-url>`. This shows the parsed
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

- `cineo -v catalog <url> <type> <id> [--extra k=v]`. It refuses
  undeclared catalogs and invalid extras **before** sending a request, and
  the message says which rule failed.
- Skipped items appear as `warn` lines with `metas[i]` locations.
- An empty result with no warnings means the addon returned
  `"metas": []` or `null`.

### Stream failures

- Streams load per addon. A failing addon shows its own error and does not
  hide the others.
- Torrent streams go through `cineo-stream`; they show a notice instead when
  P2P is turned off in Settings. Other non-`http(s)` sources (archives, NZB,
  …) are listed but refused with a notice naming the source kind.
- Logs: `RUST_LOG=cineo_net=debug` shows each `addon_request` span with its
  resource path; `RUST_LOG=cineo_stream=debug` shows player connections and
  proxy handshakes of the torrent engine.

### Playback failures

- With debug logging (`RUST_LOG=cineo_player_mpv=debug`), every IPC command
  sent is logged, and so is every `end-file` reason.
- mpv's own log is not captured yet (mpv runs with `--terminal=no`). To
  debug mpv itself, reproduce by running `mpv <url>` directly.
- Common causes: an unsupported codec (check `hwdec` fallback in the mpv log),
  an HTTP 403 (check `proxyHeaders`), an expired stream URL.

### Persistence problems

- `cineo doctor` prints the database path, schema version, row counts and an
  integrity-check result.
- Migrations log `from → to` at `info`. All pending migrations run in one
  transaction, so a failed migration leaves the database at its previous
  version.
- A corrupt database or one from a newer Cineo is reported and never
  modified. To start over, move `cineo.db` away (keep it for the report).

### UI / core synchronization

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
