# 0023. Torrent lookahead follows the bitrate

- Status: Accepted
- Date: 2026-10-15

## Context
mpv buffers at most 30 s ahead of the playhead (`cache-secs=30`). The torrent
engine kept downloading far past that: with a stream window (ADR-0019) the
fork downloads three quarters of the window ahead of where a stream reads
(`TorrentStreams::new`, `w / 4 * 3`, VERIFIED from the fork's source at
`297cbbe0`). With the default 1 GB window that is about 768 MB, minutes of
video at common bitrates. The owner asked that no more than about 30 s of
the stream be downloaded ahead (owner decision 2026-10-15).

The engine only knows bytes. The stream's position in bytes is where mpv
reads, and mpv reads up to 30 s past the playhead (INFERRED from
`cache-secs`). The core knows the file size (`TorrentStatus::Streaming`)
and the duration (`PlaybackProgress`).

## Options considered
1. A fixed, smaller lookahead in bytes. Simple, but too long for low
   bitrates and possibly too short for high ones.
2. **A lookahead in seconds of video, turned into bytes with the average
   bitrate (file size / duration).** Needs a fork change to set the
   lookahead while a torrent plays.

## Decision
- Option 2. The fork gets `ManagedTorrent::set_stream_lookahead(Option<u64>)`
  (branch `feat/stream-window`). The value is capped at the window's
  default (three quarters of the window); `None` restores the default.
  Changing it wakes the piece requesters.
- `cineo-stream` adds `Engine::set_lookahead`.
- The core emits `Effect::SetTorrentLookahead` once both the file size and
  the duration are known, and again whenever the computed value changes.
  The lookahead is 5 s of video at the average bitrate. Since it counts
  from mpv's read position, which is up to 30 s past the playhead, the
  data held ahead of the playhead stays near 35 s at most.
- Until the duration is known (the start of playback), the default stays.

## Consequences
- A slow swarm has only mpv's 30 s as a cushion, so playback may wait more
  often than before (INFERRED).
- The average bitrate underestimates peaks of a variable-bitrate file; the
  fork always queues the piece under the read position, so a short
  lookahead never stops the stream, it only slows it down.
- The fork carries a third change to port on upstream updates.
- Docs: COMPATIBILITY.md, DEVELOPMENT.md (fork commit), ADR-0019 (the
  lookahead is no longer fixed at three quarters of the window).

## Rejected alternatives
- Option 1: the owner chose 30 s of real playback time over a byte size.
