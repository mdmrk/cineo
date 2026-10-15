# 0024. A two-minute buffer

- Status: Accepted
- Date: 2026-10-15

## Context
ADR-0023 set mpv's buffer to 30 s (`cache-secs=30`) and made torrent
downloads fill up to 30 s and resume below 25 s. The owner asked for a
2-minute buffer, resuming below 100 s (owner decision 2026-10-15). A
first version resumed below 115 s; the owner saw the stream stutter a
little with it and asked for 100 s (owner report 2026-10-15, cause
UNKNOWN; INFERRED: a refill that starts earlier has more time to finish
before the buffer runs low, and asks for more pieces at once).

mpv stops reading ahead at whichever limit comes first: `cache-secs` or
`demuxer-max-bytes` (VERIFIED, mpv 0.41 manual). `demuxer-max-bytes`
defaults to 150 MiB (VERIFIED, `mpv --list-options`, 0.41), which holds
2 minutes only up to about 10 Mbps. Above that, mpv would never report
119 s, and the torrent would keep filling without stopping (INFERRED from
ADR-0023's hysteresis).

## Options considered
1. Keep `demuxer-max-bytes` at its default: less memory, but the buffer is
   shorter than 2 minutes above about 10 Mbps.
2. **Raise `demuxer-max-bytes` to 512 MiB:** 2 minutes up to about 35 Mbps;
   up to about 0.5 GiB of memory per playback.
3. Raise it to 1 GiB: 2 minutes up to about 70 Mbps; up to 1 GiB of memory.

## Decision
- Option 2. mpv gets `cache-secs=120` and `demuxer-max-bytes=512MiB`.
- The torrent hysteresis of ADR-0023 stays, with new numbers: fill up to
  120 s, stop once mpv holds 119 s, resume below 100 s.

## Consequences
- mpv's buffer moves between about 100 and 120 s. Each refill asks for
  about 20 s of video, so more pieces and peers work in parallel.
- Above about 35 Mbps (4K remuxes) mpv holds less than 2 minutes and the
  torrent keeps filling past it, by at most 120 s of video at the average
  bitrate.
- The fork caps the lookahead at three quarters of the window (ADR-0023).
  With the smallest in-memory window (256 MB, 192 MB lookahead), 2 minutes
  fit up to about 12 Mbps; above that the torrent fills less than asked
  (INFERRED).
- Playback uses up to 512 MiB more memory for mpv's forward cache, on top
  of the torrent window.
- Docs: COMPATIBILITY.md; ADR-0023 is superseded in its numbers.

## Rejected alternatives
- Option 1: the 2-minute buffer would only hold for low-bitrate streams.
- Option 3: twice the memory for the few streams above 35 Mbps.
