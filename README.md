# Cineo

**An independent, local-first desktop media center in Rust, compatible with
Stremio-protocol addons.**

Install addons by URL, browse their catalogs, open details, pick a stream and
play it in [mpv]. Your library and watch progress stay on your machine.

> **Status: pre-alpha. Engineering foundation only (Milestone 0). Nothing is
> usable yet.** See [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) for what
> works. Every feature there is marked with an evidence-based status.

## Why

The addon ecosystem around the public Stremio addon protocol is large and
open. Cineo is a clean-room, well-tested Rust client for it. It is fast and
native. It treats addon data as untrusted by default. Its architecture is
documented decision by decision.

## Planned platforms

Linux and Windows first, macOS best-effort. Mobile, TV and web are postponed.
Details: [docs/GOALS.md](docs/GOALS.md).

## Quick start (developers)

```sh
git clone <this repo> && cd cineo
scripts/check.sh        # format, lint, test, docs, dependency policy
```

The toolchain is pinned in `rust-toolchain.toml`. More in
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Architecture at a glance

A pure Rust core (protocol, domain, state; no IO), IO crates (HTTP with an
SSRF-safe network policy, an mpv player process, SQLite), and thin shells (a
CLI first, then a desktop GUI). See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
and the [decision records](docs/DECISIONS.md).

## Documentation

| | |
|-|-|
| [Goals & scope](docs/GOALS.md) | [Roadmap](docs/ROADMAP.md) |
| [Architecture](docs/ARCHITECTURE.md) | [Decisions / ADRs](docs/DECISIONS.md) |
| [Addon protocol](docs/ADDON_PROTOCOL.md) | [Compatibility](docs/COMPATIBILITY.md) |
| [Security](docs/SECURITY.md) | [Testing](docs/TESTING.md) |
| [Debugging](docs/DEBUGGING.md) | [Legal & provenance](docs/LEGAL.md) |
| [Contributing](CONTRIBUTING.md) | [Agent instructions](AGENTS.md) |

## License

Licensed under the [MIT license](LICENSE).

## Legal

Cineo is **not affiliated with, endorsed by, or sponsored by Stremio**.
"Stremio" is used only to describe protocol compatibility. Cineo ships no
content and no content addons; addons are third-party services chosen by the
user. See [docs/LEGAL.md](docs/LEGAL.md).

[mpv]: https://mpv.io
