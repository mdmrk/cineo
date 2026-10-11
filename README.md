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

- Install addons by URL
- Browse catalogs, search and view details
- Play streams in the app window, including torrents
- Torrents stream from memory by default, holding only the part around
  the playback position (1 GB, adjustable), so nothing is written to disk
- Continue watching and a local library
- No account and no cloud

Runs on Linux, Windows and macOS.

## Install

Download a build from
[Releases](https://github.com/mdmrk/cineo/releases). Cineo plays video with
libmpv, which is not included yet, so install mpv first:

- **Arch:** `pacman -S mpv`
- **Debian/Ubuntu:** `apt install libmpv2`
- **Fedora:** `dnf install mpv-libs`
- **macOS:** `brew install mpv`
- **Windows:** put `libmpv-2.dll` from an mpv development build next to
  `cineo-desktop.exe`.

## Run from source

You need Rust and libmpv (see above).

```sh
git clone https://github.com/mdmrk/cineo && cd cineo
cargo run -p cineo-desktop --release
```

A [command-line tool](docs/DEVELOPMENT.md#command-line-tool) is also
included, mainly for debugging addons.

## Learn more

- [What works](docs/COMPATIBILITY.md)
- [Roadmap](docs/ROADMAP.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Development](docs/DEVELOPMENT.md)
- [Security](docs/SECURITY.md)

## License

[MIT](LICENSE). Cineo is not affiliated with Stremio. It ships no content
and no content addons. See [Legal](docs/LEGAL.md).
