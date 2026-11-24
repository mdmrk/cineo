<img src="crates/cineo-desktop/assets/brand/icon.png" alt="Cineo logo" width="128">

# Cineo

[![Release](https://img.shields.io/github/v/release/mdmrk/cineo)](https://github.com/mdmrk/cineo/releases/latest)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20windows%20%7C%20macos%20%7C%20android-blue)](docs/COMPATIBILITY.md#platforms)
[![License](https://img.shields.io/github/license/mdmrk/cineo)](LICENSE)

A fast, native media center for Linux, Windows, macOS and Android phones.
It works with Stremio addons. No account, no cloud.

[Install](#install) · [Uninstall](#uninstall) · [Build](#build) · [License](#license)

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

<table>
  <tr>
    <td><img src="fastlane/metadata/android/en-US/images/phoneScreenshots/1.jpg" alt="Cineo home screen on Android"></td>
    <td><img src="fastlane/metadata/android/en-US/images/phoneScreenshots/2.jpg" alt="A movie page with its streams on Android"></td>
    <td><img src="fastlane/metadata/android/en-US/images/phoneScreenshots/5.png" alt="Installed and suggested addons on Android"></td>
    <td><img src="fastlane/metadata/android/en-US/images/phoneScreenshots/6.png" alt="Player and language settings on Android"></td>
  </tr>
</table>

## Install

Linux (needs mpv):

```sh
curl -fsSL https://github.com/mdmrk/cineo/releases/latest/download/install.sh | sh
```

Windows:

```powershell
irm https://github.com/mdmrk/cineo/releases/latest/download/install.ps1 | iex
```

macOS and Android: [latest release](https://github.com/mdmrk/cineo/releases/latest).

## Uninstall

Linux:

```sh
curl -fsSL https://github.com/mdmrk/cineo/releases/latest/download/install.sh | sh -s -- --uninstall
```

Windows:

```powershell
$env:CINEO_UNINSTALL = 1; irm https://github.com/mdmrk/cineo/releases/latest/download/install.ps1 | iex
```

macOS: drag Cineo from Applications to the Trash.

Android: uninstall it like any other app. This also removes its data.

On the desktop, your addons and library are kept. To remove them too, delete:

| Platform | Folders |
| --- | --- |
| Linux | `~/.local/state/cineo`, `~/.cache/cineo` |
| Windows | `%LOCALAPPDATA%\Cineo` |
| macOS | `~/Library/Application Support/Cineo`, `~/Library/Caches/Cineo` |

## Build

```sh
cargo run --release
```

Needs Rust and libmpv. See [Development](docs/DEVELOPMENT.md).

## License

[MIT](LICENSE). Not affiliated with Stremio.
