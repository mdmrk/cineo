# 0023. Torrent downloads keep 25 to 30 s ahead of the playhead

- Status: Accepted (30 s and 25 s: superseded by 0024, now 120 s and 100 s)
- Date: 2026-10-15

## Context
mpv buffers at most 30 s ahead of the playhead (`cache-secs=30`). The torrent
engine kept downloading far past that: with a stream window (ADR-0019) the
fork downloads three quarters of the window ahead of where a stream reads
(`TorrentStreams::new`, `w / 4 * 3`, VERIFIED from the fork's source at
`297cbbe0`). With the default 1 GB window that is about 768 MB, minutes of
video at common bitrates.

The owner asked for this model (owner decision 2026-10-15): the window is
the cache, in memory or on disk at its configured size, and from the
playhead on no more than 30 s are downloaded. Downloads come in bursts:
fill up to 30 s, stop, and fill again once the buffer drops below 25 s.

A first version kept a fixed 5 s past mpv's read position. The engine then
always fetched the piece mpv was reading, so mpv refilled continuously and
its buffer stayed at 30 s, and pieces came almost one at a time
(INFERRED from the fork's queue code).

The engine only knows bytes; a stream's position is where mpv reads. mpv
knows how many seconds it holds past the playhead
(`demuxer-cache-duration`), and since it reads whatever the engine has,
that is how far ahead the torrent is (INFERRED). The core knows the file
size (`TorrentStatus::Streaming`) and the duration (`PlaybackProgress`).

## Options considered
1. A fixed lookahead past mpv's read position (the first version).
2. **Hysteresis on mpv's buffer:** while filling, the lookahead is the
   seconds missing to 30 s, at the average bitrate (file size / duration);
   once mpv holds 29 s or more, the lookahead is 0 and nothing downloads;
   below 25 s, filling starts again.

## Decision
- Option 2. The fork has `ManagedTorrent::set_stream_lookahead(Option<u64>)`
  (branch `feat/stream-window`), capped at three quarters of the window.
  `Some(0)` queues nothing for streams, not even the piece a stream waits
  for. `None` restores the default.
- `cineo-player` reports mpv's buffered seconds as `PlayerEvent::Buffered`
  from its event thread, whenever the whole second changes, so it arrives
  even while the window does not repaint. The desktop forwards it as
  `Action::PlaybackBuffered`.
- The core keeps the filling state per torrent and emits
  `Effect::SetTorrentLookahead` when the value changes; `cineo-stream`'s
  `Engine::set_lookahead` applies it.
- Until the size, the duration and the buffer are known, the default
  lookahead stays.
- **Amended 2026-11-15 (owner decision): the bitrate is the larger of the
  average and the one measured in mpv's buffer.** `cineo-player` reads
  `fw-bytes` and `cache-duration` from one `demuxer-cache-state` value and,
  once mpv holds 10 s, reports bytes per second with
  `PlayerEvent::Buffered`. A stream without a known duration then uses the
  measured bitrate alone instead of the default lookahead. The lookahead
  state now lives in `cineo-stream`'s torrent task (`Lookahead`,
  `PlayerFeed`), not in core effects.

## Consequences
- mpv's buffer moves between about 25 and 30 s instead of staying at 30 s.
  Each refill asks for several seconds at once, so several pieces and peers
  work in parallel.
- A swarm slower than the video drains the buffer whatever the lookahead;
  25 s is the cushion left when a refill starts.
- If buffer reports stopped arriving while the lookahead is 0, playback
  would wait for ever. They come from mpv's event thread, which runs as long
  as mpv does.
- The average bitrate is an estimate: in a high-bitrate scene a refill
  covers fewer seconds than asked, and the next buffer report asks again.
  Since 2026-11-15 the measured bitrate covers that case once mpv holds
  10 s. mpv's `fw-bytes` counts its own per-packet overhead (VERIFIED:
  24.5 kB/s reported for a 16 kB/s WAV, libmpv 0.41), so a refill can
  reach somewhat past 120 s, most with small packets; the lookahead only
  ever grows from the average, and the fork still caps it at three
  quarters of the window.
- The fork carries a third change to port on upstream updates.
- Docs: COMPATIBILITY.md, DEVELOPMENT.md (fork commit), ADR-0019 (the
  lookahead is no longer fixed at three quarters of the window).

## Rejected alternatives
- Option 1: no bursts, the buffer never moves, and pieces come one at a
  time.
