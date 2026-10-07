# Cineo

**An independent, local-first desktop media center in Rust, compatible with
Stremio-protocol addons.**

Install addons by URL, browse their catalogs, open details, pick a stream and
play it right in the window (via libmpv). Your library and watch progress
stay on your machine.

> **Status: pre-alpha.** The desktop app browses addon catalogs, shows
> details and streams, and plays direct `http(s)` and torrent streams in
> its own window, with a local library. No installers yet. See
> [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) for what works. Every
> feature there is marked with an evidence-based status.

## Why

The addon ecosystem around the public Stremio addon protocol is large and
open. Cineo is a clean-room, well-tested Rust client for it. It is fast and
native. It treats addon data as untrusted by default. Its architecture is
documented decision by decision.

## Planned platforms

Linux and Windows first, macOS best-effort. Mobile, TV and web are postponed.
Details: [docs/ROADMAP.md](docs/ROADMAP.md).

## Quick start (developers)

```sh
git clone <this repo> && cd cineo
scripts/check.sh        # format, lint, test, docs, dependency policy
cargo run -p cineo-desktop --release   # the desktop app (needs libmpv to play)
```

The toolchain is pinned in `rust-toolchain.toml`. More in
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Command-line tool

A headless front end, mainly for debugging addons. Logs go to stderr: use
`-v` for debug, `-vv` for trace, or set `RUST_LOG`.

```sh
cineo addon inspect <manifest-url>                 # validate + summarize, list warnings
cineo catalog <manifest-url> <type> <catalog-id> [--extra name=value]...
cineo addon add <manifest-url>                     # validate, then install at the end
cineo addon remove <manifest-url>
cineo addon list                                   # installed addons, in order
cineo library [--all]                              # continue watching (or every item)
cineo doctor                                       # database path, schema, counts, integrity
```

Examples:

```sh
cineo addon inspect https://v3-cinemeta.strem.io/manifest.json
cineo catalog https://v3-cinemeta.strem.io/manifest.json movie top --extra genre=Drama
```

Output of `catalog`: one tab-separated line per item: `id  type  year  name`.
Output of `library`: `id  video-id  position/duration  name`, most recent
first.

Installed addons and the library live in one SQLite database, by default in
`~/.local/share/cineo/cineo.db` (Linux), `%APPDATA%\Cineo\data\cineo.db`
(Windows) or `~/Library/Application Support/Cineo/cineo.db` (macOS).
`--data-dir <dir>` uses another directory. `addon list` prints full addon
URLs, which can contain your addon configuration; do not paste them publicly.
A database that is corrupt or from a newer Cineo is never modified: commands
fail with an explanation, and `cineo doctor` describes the problem.

`--allow-private-network` permits addons on localhost or the LAN. It is off
by default; see [docs/SECURITY.md](docs/SECURITY.md).

## Architecture at a glance

A pure Rust core (protocol, domain, state; no IO), IO crates (HTTP with an
SSRF-safe network policy, an in-window libmpv player, a torrent engine,
SQLite), and thin shells (a
CLI and an egui desktop GUI). See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
and the [decision records](docs/adr/README.md).

## Documentation

| | |
|-|-|
| [Goals, scope & roadmap](docs/ROADMAP.md) | [Compatibility](docs/COMPATIBILITY.md) |
| [Architecture](docs/ARCHITECTURE.md) | [Decisions (ADRs)](docs/adr/README.md) |
| [Addon protocol](docs/ADDON_PROTOCOL.md) | [Security](docs/SECURITY.md) |
| [Development: setup, testing, debugging, releasing](docs/DEVELOPMENT.md) | [Technical debt](docs/TECHNICAL_DEBT.md) |
| [Legal & provenance](docs/LEGAL.md) | [Agent instructions](AGENTS.md) |

## License

Licensed under the [MIT license](LICENSE).

## Legal

Cineo is **not affiliated with, endorsed by, or sponsored by Stremio**.
"Stremio" is used only to describe protocol compatibility. Cineo ships no
content and no content addons; addons are third-party services chosen by the
user. See [docs/LEGAL.md](docs/LEGAL.md).

