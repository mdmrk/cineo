# 0016. librqbit from the owner's fork instead of a vendored copy

- Status: Accepted
- Date: 2026-10-07

## Context
ADR-0015 vendored `librqbit` 9.0.1 in `vendor/librqbit` to carry our
streaming patch (urgent pieces from several peers). That put about 27,000
lines of third-party code in the repository, outside our lints, and made
updates a manual port. The owner pushed the patch to a fork as branch
`fix/urgent-piece-helpers` (https://github.com/mdmrk/rqbit): upstream `main` plus one commit
(VERIFIED 2026-10-07: `be52c57a`, the same change as the vendored one
apart from test names). No upstream PR does the same (VERIFIED
2026-10-07: all 397 PRs of `ikatson/rqbit` scanned).

## Options considered
1. Keep the vendored copy.
2. **The fork as a git dependency through `[patch.crates-io]`, pinned to a
   commit.**
3. The fork pinned to the branch name. Builds would change whenever the
   branch moves.

## Decision
- Option 2. The root `Cargo.toml` patches `librqbit` with
  `git = "https://github.com/mdmrk/rqbit"` and `rev` set to a commit of `fix/urgent-piece-helpers`.
  `.config/deny.toml` allows that one git source. `vendor/` is removed.
- The fork's branch is upstream `main`, not the 9.0.1 release. So we also
  get the upstream fixes merged since 9.0.1 (25 commits on 2026-10-07, for
  example the configurable per-peer request window, #687). Our tests run
  against it.
- Updating: rebase the branch on upstream, push, change `rev`, run
  `scripts/check.sh`. The goal is to drop the fork once upstream releases
  the patch.

## Consequences
- Builds need access to GitHub (or a cargo git cache) as well as
  crates.io.
- The librqbit sibling crates (`librqbit-core`, `-dht`, …) also come
  from the fork, since they are path dependencies in its workspace.
- The patch's own unit tests live in the fork; ours
  (`a_slow_peer_does_not_hold_back_the_start`) still check the behavior.
- ADR-0015's other decisions (no torrent cache, carrying the patch) stand.
- 2026-10-09: the pinned commit moved to `297cbbe0` on branch
  `feat/stream-window`, which adds the stream window of ADR-0019 on top of
  `fix/urgent-piece-helpers`.

## Rejected alternatives
- Option 1: the owner asked to use the fork; the copy was large and
  updates were manual.
- Option 3: an unpinned branch makes builds unreproducible.
