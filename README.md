# Cineo

[![Release](https://img.shields.io/github/v/release/mdmrk/cineo)](https://github.com/mdmrk/cineo/releases/latest)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20windows%20%7C%20android-blue)](docs/COMPATIBILITY.md#platforms)
[![License](https://img.shields.io/github/license/mdmrk/cineo)](LICENSE)

A fast, native media center for Linux, Windows and Android phones.
It works with Stremio addons. No account, no cloud.

<table>
  <tr>
    <td><img src="docs/images/home.png" alt="Cineo home screen"></td>
    <td><img src="docs/images/detail.png" alt="A movie page with its streams"></td>
  </tr>
  <tr>
    <td><img src="docs/images/addons.png" alt="Installed addons"></td>
    <td><img src="docs/images/player.png" alt="A movie playing"></td>
  </tr>
</table>

## Features

- Stremio addons, catalogs and search
- Built-in player with torrent streaming
- Subtitles, binge watching and a local library
- Everything stays on your device

## Install

### Linux (x86_64)

Install your distribution's **mpv** package first, then run:

```sh
curl -fsSL https://github.com/mdmrk/cineo/releases/latest/download/install.sh | sh
```

### Windows (x86_64)

In PowerShell:

```powershell
irm https://github.com/mdmrk/cineo/releases/latest/download/install.ps1 | iex
```

Nothing else is needed.

### Android (arm64 phones)

Download and open `cineo-arm64-android.apk` from the
[latest release](https://github.com/mdmrk/cineo/releases/latest).
The Android app does less than the desktop one; see
[what works](docs/COMPATIBILITY.md#platforms).

## Update

Run the install command again. On Android, install the new APK over the
old one.

## Uninstall

Linux:

```sh
curl -fsSL https://github.com/mdmrk/cineo/releases/latest/download/install.sh | sh -s -- --uninstall
```

Windows (PowerShell):

```powershell
$env:CINEO_UNINSTALL=1; irm https://github.com/mdmrk/cineo/releases/latest/download/install.ps1 | iex
```

Your addons and library are kept. To remove them too, delete
`~/.local/state/cineo` (Linux) or `%LOCALAPPDATA%\Cineo\data` (Windows).
Earlier versions kept them in `~/.local/share/cineo` or `%APPDATA%\Cineo`,
and Cineo does not move them.

On Android, uninstall Cineo like any other app. This also deletes your
addons and library.

## For developers

```sh
git clone https://github.com/mdmrk/cineo && cd cineo
cargo run -p cineo-desktop --release
```

You need [Rust](https://rustup.rs) and libmpv. Windows and Android builds
are covered in [Development](docs/DEVELOPMENT.md). See also
[Architecture](docs/ARCHITECTURE.md), the
[addon protocol](docs/ADDON_PROTOCOL.md), [Security](docs/SECURITY.md),
[known limitations](docs/TECHNICAL_DEBT.md) and the
[decision records](docs/adr/README.md).

## License

[MIT](LICENSE). Cineo is not affiliated with Stremio and ships no content
or content addons. See [Legal](docs/LEGAL.md).
