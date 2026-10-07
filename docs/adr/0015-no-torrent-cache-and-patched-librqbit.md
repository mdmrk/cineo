# 0015. No torrent cache; a vendored, patched librqbit

- Status: Accepted
- Date: 2026-10-07

## Context
Two problems with the torrent engine from ADR-0010 and ADR-0012:

- **Disk use.** ADR-0010 bounded the torrent cache; we implemented it as
  5 GiB with least-recently-used eviction. The owner found it took too much
  disk (owner decision 2026-10-07: remove caching).
- **Slow starts and seeks.** `librqbit` 9.0.1 (the latest release on
  2026-10-07) asks one peer per piece (VERIFIED from its source,
  `piece_tracker.rs`). It moves a piece to another peer only once the
  piece took 10× (later 3×) that peer's average time. So a slow peer
  holding one of the next pieces stalls playback. Stremio's engine asks
  several peers for the pieces it needs next (INFERRED from its
  behavior). `librqbit` has no option for this.
  Measured (VERIFIED, 2026-10-07, release build, local seeders, 512 KiB
  pieces, one seeder throttled to 128 KiB/s): the first 2 MiB took 5.1 s
  in 7 of 8 runs.

## Options considered
1. Keep a smaller cache. Still uses disk for data the user may never play
   again.
2. **No cache:** delete a torrent's data when it stops.
3. Switch to libtorrent-rasterbar, whose `set_piece_deadline` is built for
   streaming. C++ through FFI with our own bindings, a Boost/OpenSSL build
   on every platform, and a rewrite of `cineo-stream`.
4. **Patch librqbit** (Apache-2.0) to ask several peers for urgent pieces,
   vendored and used through `[patch.crates-io]`.
5. Wait for upstream. No such change is planned that we know of
   (UNKNOWN).

## Decision
- **No cache (option 2).** Only the torrent being played is on disk. Its
  directory is deleted when it stops (Back, another stream, quitting).
  Directories named like an info hash are deleted when the engine starts,
  in case a run did not stop cleanly. `dht.json` stays. This replaces
  ADR-0010's bounded cache; the `<cache>/<info hash>/<file index>` layout
  from ADR-0012 stays.
- **Vendored, patched librqbit (option 4).** `vendor/librqbit` holds the
  published 9.0.1 crate with our changes. Each changed file carries a
  notice, and `vendor/README.md` lists every change and how to port it.
  The first change: the next 4 pieces a stream needs may get up to 2
  helper peers besides their owner; whoever completes the piece first
  finishes it.

## Consequences
- Playing a torrent again downloads it again.
- A file larger than the free space can still fill the disk
  (TECHNICAL_DEBT.md).
- Every librqbit update means porting our changes. The vendored crate is
  not a workspace member: its own tests run separately
  (`vendor/README.md`), and our lints do not apply to it.
- Shared urgent pieces cost some duplicate download, and a bad chunk from
  a helper can get the wrong peer disconnected (TECHNICAL_DEBT.md).
- Docs: SECURITY.md (disk exhaustion), LEGAL.md (adapted code),
  DEVELOPMENT.md (dependencies).

## Rejected alternatives
- Option 1: the owner asked for no caching.
- Option 3: months of FFI and build work. Re-measure with real swarms
  before reconsidering it.
- Option 5: leaves playback stalling until then.
