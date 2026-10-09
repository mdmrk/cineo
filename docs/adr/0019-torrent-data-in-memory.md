# 0019. Torrent data in memory, bounded by a sliding window

- Status: Accepted
- Date: 2026-10-09

## Context
ADR-0015 keeps only the torrent being played on disk, but the whole file
builds up there until playback stops. The owner asked for an option to
keep torrent data in RAM instead (2026-10-09), and chose a bounded window
over keeping the whole file in memory.

`librqbit` cannot do this on its own (VERIFIED from the fork's source at
`be52c57a`):
- once a piece passes its hash check it is "have" for good
  (`ChunkTracker::mark_piece_downloaded`); nothing marks it missing again;
- it downloads every selected piece in order, besides the 32 MiB a stream
  asks for first (`PieceTracker::acquire_piece`, `PER_STREAM_BUF_DEFAULT`);
- its example in-memory storage (`storage/examples/inmemory.rs`) never
  frees a piece.

## Options considered
1. Whole file in memory, with disk as the fallback above a size cap. No
   librqbit change, but memory grows to the file size.
2. **A sliding window:** a byte budget around the playback position;
   pieces outside it are dropped, and downloaded again if a stream reaches
   them. This needs a librqbit change.

## Decision
- Option 2. The fork (ADR-0016) gets `AddTorrentOptions::stream_window`
  (branch `feat/stream-window`, commit `297cbbe0`, on top of
  `fix/urgent-piece-helpers`). With a window:
  - only pieces that streams read next are downloaded, three quarters of
    the window ahead of each stream;
  - after each completed piece, if the pieces held exceed the window, the
    ones farthest from every stream are forgotten
    (`ChunkTracker::forget_piece`) and handed to the new
    `TorrentStorage::discard_piece`. A piece in a stream's read-ahead is
    never forgotten, and nothing is while no stream is open;
  - a peer request for a forgotten piece is ignored, not answered with a
    disconnect: BitTorrent has no message that takes back a "have";
  - idle peers wake when a stream moves to another piece.
- `cineo-stream` adds `MemoryStorage` (one buffer per piece) and
  `EngineOptions::memory_window`. The engine's own default is disk.
- Settings → Torrents → "Torrent data": on disk, or in memory with a
  256 MB, 512 MB, 1 GB or 2 GB window. In memory with 1 GB is the default
  (owner decision 2026-10-09). It applies from the next
  torrent.

## Consequences
- In memory, nothing of the torrent is written to disk (`dht.json` still
  is). Seeking back past the window downloads that part again.
- Memory can exceed the window by the pieces in flight. Partial pieces
  left by peers that died stay in memory until the torrent stops
  (TECHNICAL_DEBT.md).
- A forgotten piece can still be in a peer's upload queue; reading it
  fails and that peer is disconnected (TECHNICAL_DEBT.md).
- The torrent progress in the stats overlay shows the bytes held, not
  the bytes downloaded so far.
- The fork now carries two changes to port on every upstream update.
- Docs: SECURITY.md (memory instead of disk), COMPATIBILITY.md (settings),
  DEVELOPMENT.md (fork commit).

## Rejected alternatives
- Option 1: the owner chose the window; RAM use equal to the file size
  is too much for large files.
