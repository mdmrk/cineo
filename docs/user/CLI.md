# `cineo` command-line tool

A headless front end, mainly for debugging addons. Logs go to stderr: use
`-v` for debug, `-vv` for trace, or set `RUST_LOG`.

```sh
cineo addon inspect <manifest-url>                 # validate + summarize, list warnings
cineo catalog <manifest-url> <type> <catalog-id> [--extra name=value]...
```

Examples:

```sh
cineo addon inspect https://v3-cinemeta.strem.io/manifest.json
cineo catalog https://v3-cinemeta.strem.io/manifest.json movie top --extra genre=Drama
```

Output of `catalog`: one tab-separated line per item: `id  type  year  name`.

`--allow-private-network` permits addons on localhost or the LAN. It is off
by default; see `docs/SECURITY.md`.
