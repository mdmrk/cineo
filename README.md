# Cineo

A fast, native desktop media center written in Rust. It works with
Stremio-protocol addons.

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

## Features

- Stremio addons, catalogs and search
- Built-in player with torrent streaming
- Subtitles, binge watching and a local library
- No account, no cloud

Linux x86_64 and Windows x86_64. Windows builds have not been tried on a
real machine yet.

## Install and update

```sh
curl -fsSL https://github.com/mdmrk/cineo/releases/latest/download/install.sh | sh
```

Cineo needs libmpv (the mpv package of your distribution). Or download
the executable from [Releases](https://github.com/mdmrk/cineo/releases)
and run it. To uninstall: `… | sh -s -- --uninstall`.

On Windows (PowerShell):

```powershell
irm https://github.com/mdmrk/cineo/releases/latest/download/install.ps1 | iex
```

The Windows executable carries libmpv. To uninstall, run the same command
with `$env:CINEO_UNINSTALL=1`.

## Run from source

You need Rust and libmpv.

```sh
git clone https://github.com/mdmrk/cineo && cd cineo
cargo run -p cineo-desktop --release
```

A [command-line tool](docs/DEVELOPMENT.md#command-line-tool) is also
included, mainly for debugging addons.

## Learn more

- [What works](docs/COMPATIBILITY.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Development](docs/DEVELOPMENT.md)
- [Security](docs/SECURITY.md)

## License

[MIT](LICENSE). Cineo is not affiliated with Stremio. It ships no content
and no content addons. See [Legal](docs/LEGAL.md).
