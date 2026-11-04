# Cineo

[![CI](https://img.shields.io/github/actions/workflow/status/mdmrk/cineo/ci.yml?branch=main&label=ci)](https://github.com/mdmrk/cineo/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/mdmrk/cineo)](https://github.com/mdmrk/cineo/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/mdmrk/cineo/total)](https://github.com/mdmrk/cineo/releases)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20windows%20%7C%20android-blue)](docs/COMPATIBILITY.md#platforms)
[![License](https://img.shields.io/github/license/mdmrk/cineo)](LICENSE)

A fast, native media center written in Rust, for the desktop and Android
phones. It works with Stremio-protocol addons.

<table>
  <tr>
    <td><img src="docs/images/home.png" alt="Cineo home screen"></td>
    <td><img src="docs/images/detail.png" alt="A movie page with its streams"></td>
  </tr>
  <tr>
    <td><img src="docs/images/addons.png" alt="Installed addons"></td>
    <td><img src="docs/images/settings.png" alt="Torrent settings"></td>
  </tr>
</table>

[Features](#features) · [Install](#install) · [Uninstall](#uninstall) ·
[Build from source](#build-from-source) ·
[Documentation](#documentation) · [License](#license)

## Features

- Stremio addons, catalogs and search
- Built-in player with torrent streaming
- Subtitles, binge watching and a local library
- No account, no cloud

## Install

Run the same command again to update. Executables for every release are on
the [Releases](https://github.com/mdmrk/cineo/releases) page.

### Linux (x86_64)

```sh
curl -fsSL https://github.com/mdmrk/cineo/releases/latest/download/install.sh | sh
```

Cineo needs libmpv: install your distribution's mpv package.

### Windows (x86_64)

In PowerShell:

```powershell
irm https://github.com/mdmrk/cineo/releases/latest/download/install.ps1 | iex
```

libmpv is built in.

### Android (arm64 phones)

Install `cineo-arm64-android.apk` from the
[latest release](https://github.com/mdmrk/cineo/releases/latest). It does
less than the desktop app (see [Compatibility](docs/COMPATIBILITY.md#platforms)).

## Uninstall

Linux:

```sh
curl -fsSL https://github.com/mdmrk/cineo/releases/latest/download/install.sh | sh -s -- --uninstall
```

Windows (PowerShell):

```powershell
$env:CINEO_UNINSTALL=1; irm https://github.com/mdmrk/cineo/releases/latest/download/install.ps1 | iex
```

Your addons and library are kept. To delete them too, remove
`~/.local/share/cineo` on Linux or `%APPDATA%\Cineo` on Windows.

Android: uninstall Cineo like any other app. This also deletes its addons
and library.

## Build from source

You need [Rust](https://rustup.rs) and libmpv.

```sh
git clone https://github.com/mdmrk/cineo && cd cineo
cargo run -p cineo-desktop --release
```

On Windows, put a `libmpv-2.dll` next to the executable
([details](docs/DEVELOPMENT.md)). A
[command-line tool](docs/DEVELOPMENT.md#command-line-tool) is also
included, mainly for debugging addons. The Android app builds with the
NDK and Gradle; see [Development](docs/DEVELOPMENT.md).

## Documentation

| Document | Contents |
|----------|----------|
| [Compatibility](docs/COMPATIBILITY.md) | What works, per feature and platform |
| [Addon protocol](docs/ADDON_PROTOCOL.md) | How Cineo reads Stremio addons |
| [Architecture](docs/ARCHITECTURE.md) | Crates, data flow and platform code |
| [Development](docs/DEVELOPMENT.md) | Building, testing and releasing |
| [Security](docs/SECURITY.md) | Threat model and defaults |
| [Technical debt](docs/TECHNICAL_DEBT.md) | Known limitations |
| [Decisions](docs/adr/README.md) | Architecture decision records |

## License

[MIT](LICENSE). Cineo is not affiliated with Stremio. It ships no content
and no content addons. See [Legal](docs/LEGAL.md).
