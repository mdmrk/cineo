# `cineo` command-line tool

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
by default; see `docs/SECURITY.md`.
