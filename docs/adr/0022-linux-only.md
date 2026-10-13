# 0022. Linux only

- Status: Accepted
- Date: 2026-10-13
- Supersedes: the Windows and macOS parts of 0014 (bundled libmpv) and
  0018 (named pipe, registry registration, macOS links)

## Context
Cineo shipped builds for Linux x86_64, Windows x86_64 and macOS arm64. Only
Linux was ever run by hand: embedded playback, the Windows named pipe,
registry scheme registration, `install.ps1` and the bundled libmpv were
never exercised on a Windows or macOS machine (TECHNICAL_DEBT.md, before
this change). The Windows build also carried an LGPL libmpv with its own
fetch, mirror and notice scripts. The software is feature complete; the
owner decided on 2026-10-13 to drop Windows and macOS.

## Options considered
1. Keep all three platforms — the untested code stays in the release and
   CI keeps three runners and the libmpv mirror.
2. Linux only — less code, one runner, no bundled LGPL binary.

## Decision
Cineo supports Linux x86_64 only.
- CI tests and release builds run on `ubuntu-latest` only; releases carry
  `cineo-x86_64-linux`, `cineo.png`, `install.sh`, `cineo-licenses.zip`
  and `SHA256SUMS`.
- `cineo-player` looks for `libmpv.so.2`/`libmpv.so` beside the executable,
  then on the system path. No libmpv is bundled; the `bundled-libmpv`
  feature, `libmpv.rs` and the libmpv fetch and mirror scripts are removed.
- The single-instance handover uses a Unix socket only; scheme
  registration uses `xdg-mime` only.
- Code may use `std::os::unix` without `cfg` guards.
- `cargo deny` checks the `x86_64-unknown-linux-gnu` graph only.

## Consequences
- Less platform code and no LGPL binary in releases; `cineo-licenses.zip`
  carries only Cineo's and the fonts' licenses.
- The crates no longer build for Windows; macOS may build but is not
  tested or shipped.
- Users on other systems have no supported build.

## Rejected alternatives
- Keeping Windows and macOS: nobody runs them, so their behavior stays
  UNKNOWN while the code and CI cost remain.
