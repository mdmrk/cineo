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

- Addons by URL or from the official and community lists
- Catalogs, discover and search across addons
- Streams from all addons in one list, with filters and sorting
- Playback in the app window, torrents included, streamed from memory
- Subtitles from addons and files, with preferred languages and styling
- Binge watching, continue watching, favourites and a local library
- No account and no cloud

Linux x86_64 only.

## Install

```sh
curl -fsSL https://github.com/mdmrk/cineo/releases/latest/download/install.sh | sh
```

Cineo needs libmpv (the mpv package of your distribution). Or download
the executable from [Releases](https://github.com/mdmrk/cineo/releases)
and run it. To uninstall: `… | sh -s -- --uninstall`.

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
