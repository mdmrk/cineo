# Cineo

A fast, native desktop media center written in Rust. It works with
Stremio-protocol addons.

![Cineo home screen](docs/images/home.png)

![A movie page with its streams](docs/images/detail.png)

## Features

- Install addons by URL
- Browse catalogs, search and view details
- Play streams in the app window, including torrents
- Torrents stream from memory by default, holding only the part around
  the playback position (1 GB, adjustable), so nothing is written to disk
- Continue watching and a local library
- No account and no cloud

Runs on Linux, Windows and macOS.

## Run it

You need Rust and libmpv.

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
