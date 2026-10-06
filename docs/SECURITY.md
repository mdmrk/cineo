# Security model

Cineo is a media client that **consumes untrusted remote data by design**:
addons are arbitrary third-party servers. This document defines the threat
model and the mandatory controls. Agent rules in `.claude/rules/security.md`
enforce it during development.

Sources consulted (2026-10-06):
- OWASP SSRF Prevention Cheat Sheet.
- The mpv manual (JSON IPC, protocols, unsafe playlists).
- The stremio-shell-ng source (an anti-pattern: raw `mpv-command` passthrough).
- The reqwest 0.13 API (custom DNS resolver, redirect policy).

## Assets

- The user's machine and local network: routers, NAS, admin panels on
  `192.168.x.x`, cloud metadata endpoints.
- Addon transport URLs. They often embed **API keys and configuration**.
- Library and watch history (private).
- Future account credentials.

## Threat actors

1. **A malicious or compromised addon.** It controls every byte of its
   responses: URLs, sizes, encodings, redirects, header values, timings.
2. **A network attacker.** This applies to `http://` addons only.
3. **A malicious media or subtitle file** reaching the player.

## Untrusted inputs

These are all treated as hostile:
- Addon URLs, manifests and JSON responses.
- Metadata strings and images.
- Stream, subtitle and external URLs.
- Redirect targets and HTTP headers, including `proxyHeaders` from addons.
- Downloaded files.
- Local media files.

## Controls

### Network (`cineo-net`, M1)

| Risk | Control |
|------|---------|
| SSRF / LAN attacks: an addon (or a URL inside its data) points at `127.0.0.1`, `192.168.1.1`, `169.254.169.254` | `NetPolicy` blocks non-public destinations **by default**. IP literals and `localhost` are checked before connecting. Hostnames are checked **after DNS resolution inside the resolver**: the connection only uses checked addresses, which defeats DNS rebinding. Blocked: loopback, RFC 1918, link-local, CGNAT, multicast, broadcast, documentation, benchmarking, reserved, unspecified, plus IPv6 forms embedding IPv4 (mapped, NAT64, 6to4). |
| Self-hosted addons on the LAN | Explicit opt-in only: `--allow-private-network` (CLI) for now. Planned: a per-addon trust flag set at install time with a clear prompt. A public addon can never cause private-network requests. |
| Redirect-based bypass | Every redirect hop is re-validated: scheme, destination and https→http downgrade (refused). At most 5 hops. |
| Proxy bypasses the resolver check | System proxies are disabled for addon traffic (INFERRED necessity: a proxy resolves names itself). |
| Scheme abuse (`file:`, `ftp:`, `data:`) | Only `http`/`https` for addons and images. |
| Oversized responses / decompression bombs | Limit on **decoded** bytes (default 8 MiB), counted while streaming. `Content-Length` is checked early. |
| Slow-loris addons | Total request timeout of 20 s, connect timeout of 10 s. |
| Malformed JSON | serde_json (recursion limit 128). Lenient field-level parsing never panics. Fuzzing is planned for the parsers (TESTING.md). |
| Credential leakage | Transport URLs with userinfo are rejected. Logs and errors include only the origin and resource path, never full addon URLs. Error messages from the HTTP stack are scrubbed of URLs. |

### Player (`cineo-player-mpv`, M3)

| Risk | Control |
|------|---------|
| Option injection via the URL (`--script=…`) | Load media through the IPC `loadfile` command with a JSON array argument. Never build command lines from addon data. Never use `mpv_command_string`-style string commands. |
| Dangerous mpv protocols (`edl://`, `lavfi://`, `av://`, `file://`, `memory://`, `fd://`) | Only `http`/`https` URLs from addons reach mpv. Local files are allowed only when the **user** picked them. |
| Untrusted playlists | Do not enable `--load-unsafe-playlists`. |
| `ytdl` hook running an external program on addon URLs | Start mpv with `--ytdl=no` unless a later ADR decides otherwise. |
| User mpv config/scripts changing behavior | Start mpv with `--no-config` plus an explicit option set (INFERRED: gives reproducible behavior; revisit if users want their config). |
| Header injection via `proxyHeaders` | Header names must be RFC 7230 tokens, and values must not contain CR/LF/NUL. Otherwise the stream is rejected. |
| IPC socket hijack | The mpv manual states that IPC is not secure. Put the socket in a per-user runtime directory with `0700` permissions (Unix) and give it a random name. Windows named pipes get a random name. |
| Raw command passthrough from the UI | The UI sends typed `PlayerCommand`s only. There is no "send arbitrary mpv command" path, unlike shell-ng. |

### Subtitles

Subtitle parsing and rendering is delegated to mpv/libass. Cineo does not
parse subtitle formats itself (INFERRED: this reduces our attack surface; we
rely on mpv updates). Subtitle URLs follow the same network policy and are
fetched by mpv. Whether mpv applies our policy to them is **UNKNOWN**; this
must be resolved in M6. One option is fetching them through `cineo-net` into
a temp file.

### Filesystem and persistence (M4)

- All paths are derived from platform data directories. Addon data never
  forms a path; ids are stored as data, never used as file names.
- SQLite queries use bound parameters only.
- Secrets (future account tokens) go to the OS keyring. They are never
  written to SQLite, logs or exports.

### Process execution

No shell is ever invoked with addon-controlled input. Opening `externalUrl`
in the browser requires a user confirmation and an `http(s)` scheme.

### Web UI (M5, if Tauri is confirmed)

- Strict CSP.
- No remote scripts.
- IPC commands are an allowlist of typed actions.
- Addon-provided strings are rendered as text, never as HTML.

## Defaults summary

| Setting | Default |
|---------|---------|
| Private-network access | Off |
| Max decoded body | 8 MiB |
| Request timeout / connect timeout | 20 s / 10 s |
| Max redirects | 5, no https→http |
| mpv | `--no-config --ytdl=no`, IPC `loadfile` only, http(s) only |

## Unsafe code

`unsafe_code = "forbid"` workspace-wide. A crate that genuinely needs FFI
(for example a future libmpv embedding) must:
1. Lift the lint for that crate only.
2. Justify it in an ADR.
3. Document every `unsafe` block with a `// SAFETY:` comment.

## Reporting a vulnerability

Do not open public issues for vulnerabilities. Use GitHub's private
vulnerability reporting on this repository ("Security" tab → "Report a
vulnerability"). We aim to acknowledge reports within 7 days. There is no
supported release yet, so fixes land on `main`.
