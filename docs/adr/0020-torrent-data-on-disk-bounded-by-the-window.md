# 0020. Torrent data on disk, bounded by the same window

- Status: Accepted
- Date: 2026-10-11

## Context
With "Torrent data" on disk (ADR-0015, ADR-0019), the file being played
builds up whole on disk until playback stops, so a file larger than the
free space fills the disk. The owner asked to bound disk use to a few GB,
like memory (owner decision 2026-10-11).

The fork's `stream_window` (ADR-0019) is not tied to memory: it forgets
pieces and hands them to `TorrentStorage::discard_piece` whatever the
storage (VERIFIED from the fork's source at `297cbbe0`). The disk storage
kept one sparse file per torrent file, which cannot give space back for a
single piece without hole punching (platform-specific, `unsafe` FFI).

## Options considered
1. Refuse files larger than the free space up front. Needs a free-space
   dependency; disk use still grows to the file size.
2. **The same window on disk, one file per piece.** A forgotten piece is a
   deleted file. No new dependency, no `unsafe`, no fork change.

## Decision
- Option 2. `CacheStorage` stores `<cache>/<info hash>/<piece index>` and
  deletes that file in `discard_piece`. A lock keeps a write from
  recreating a piece file while it is deleted.
- `EngineOptions::memory_window` becomes `window` (either storage) plus
  `in_memory`. Without a window the engine keeps the whole file on disk,
  as before.
- Settings → "Torrent data": in memory with 256 MB, 512 MB, 1 GB or 2 GB,
  or on disk with 2, 4 or 8 GB. The stored value `disk` from before means
  on disk with 4 GB. In memory with 1 GB stays the default.
- Nothing is kept as a cache, as ADR-0015 decided: the data is deleted when
  playback stops. The setting's help text says so.

## Consequences
- Disk use stays near the window; it can go over by the same pieces in
  flight as memory (TECHNICAL_DEBT.md).
- Every read and write opens its piece file; no file handles are kept, so
  a large window with small pieces cannot run out of them.
- This supersedes ADR-0015's `<file index>` layout and ADR-0019's
  unbounded disk choice.
- Docs: SECURITY.md (layout, disk exhaustion), COMPATIBILITY.md
  (settings), DEVELOPMENT.md (manual test T6), TECHNICAL_DEBT.md.

## Rejected alternatives
- Option 1: it only fails earlier; the owner wanted bounded disk use.
- Hole punching in the per-file layout: needs `unsafe` FFI per platform.
