# Desktop GUI manual test

The UI tests (`crates/cineo-desktop/tests/ui.rs`) check rendering and the
actions clicks produce. This script covers what they cannot: real windows,
real addons, mpv, and restarts. Run it before marking a GUI milestone done,
on **Linux and Windows**, and record the date, OS and result in
[ROADMAP.md](../ROADMAP.md) or the PR.

## Setup

- Build: `cargo build -p cineo-desktop --release`. mpv must be on `PATH`
  (or pass `--mpv <path>`).
- Use a throwaway data directory: `--data-dir <tmp>`.
- A stream addon that returns direct `http(s)` URLs. Without one, use this
  local addon (only for testing; the video must be a file you may use):

  ```sh
  mkdir -p addon/stream/movie && cd addon
  cp /path/to/video.mp4 video.mp4
  cat > manifest.json <<'JSON'
  {"id":"org.cineo.manual","version":"1.0.0","name":"Manual Test Streams",
   "resources":["stream"],"types":["movie"],"idPrefixes":["tt"],"catalogs":[]}
  JSON
  # tt0063350 = Night of the Living Dead (1968) in Cinemeta
  echo '{"streams":[{"name":"Local","url":"http://127.0.0.1:8000/video.mp4"}]}' \
    > stream/movie/tt0063350.json
  python3 -m http.server 8000
  ```

  Then start Cineo with `--allow-private-network` so it may reach
  `127.0.0.1`.

## Steps (GOALS.md success scenario 1)

| # | Do | Expect |
|---|----|--------|
| 1 | Start `cineo-desktop --data-dir <tmp> -v` | Empty Board with "No addons installed…" |
| 2 | Addons → paste `https://v3-cinemeta.strem.io/manifest.json` → Install | "Installed Cinemeta"; Board shows rows with posters |
| 3 | Install the stream addon (`http://127.0.0.1:8000/manifest.json` for the local one) | It appears second in the list; Move up/down reorders |
| 4 | Search "Night of the Living Dead" → open the 1968 film | Detail page: backdrop, poster, facts, description |
| 5 | Streams list | One group per stream addon; the local stream has an enabled Play |
| 6 | Play | mpv opens and plays. Total time from step 2: under a minute |
| 7 | Seek to ~10 minutes, wait 10 s, close mpv | Board shows "Continue watching" with a progress bar |
| 8 | Quit Cineo, start it again with the same `--data-dir` | Addons (in order) and Continue watching are back |
| 9 | Open the item from Continue watching → Play | mpv resumes at ~10 minutes |
| 10 | Discover → pick "Popular — Cinemeta", a genre, "Load more" | Items change with the genre; more items append |
| 11 | Open a series → choose a season → an episode | Episode list for that season; streams load for the episode |
| 12 | Torrent streams | See [Torrents (M9)](#torrents-m9) |
| 13 | Library → Remove the item | It disappears, and stays gone after a restart |
| 14 | Install `http://127.0.0.1:9/manifest.json` without `--allow-private-network` | Error mentions the network policy; nothing crashes |

Also watch the `-v` log: no panics, and no full addon or stream URLs (only
origins and resource paths).

## Torrents (M9)

Use a torrent of content you may share (for example a public-domain film
from the Internet Archive; each item has a torrent whose info hash is on
its page). Add a stream to the local addon above, in front of the
`Local` entry:

```json
{"name":"Torrent","infoHash":"<40 hex characters>","fileIdx":0}
```

Use a fresh `--data-dir` and `--cache-dir` so the P2P notice appears.

| # | Do | Expect |
|---|----|--------|
| T1 | Open the film; Play the torrent stream | The "Peer-to-peer streaming" notice; nothing appears in the cache directory yet |
| T2 | Cancel | No playback, no engine log lines (`-v`) |
| T3 | Play again → Accept and play | A bottom bar "Torrent: looking for peers…", then peers, speed and percent; mpv opens and plays |
| T4 | Seek forward and back in mpv | Playback resumes within a few seconds |
| T5 | Close mpv | The bottom bar disappears; the log shows the torrent stopped |
| T6 | Play it again | No notice this time; starts faster (data in the cache) |
| T7 | Settings → untick "Show and play torrent streams"; open the film | The torrent stream is gone; "1 torrent stream hidden…" is shown |
| T8 | Restart with the same directories | The setting is kept |
| T9 | Look in the cache directory | Only `<info hash>/<file index>` files and `dht.json`; no torrent file names |
