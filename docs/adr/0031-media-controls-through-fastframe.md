# 0031. Media controls through fastframe-now-playing

- Status: Accepted
- Date: 2026-11-08
- Supersedes: the media keys decision of 0027

## Context
The desktop shell kept two backends for media keys: an MPRIS service written
on `zbus` (Linux, 270 lines) and SMTC through `souvlaki` (Windows, 123
lines). Each repeated the change tracking and the event mapping, and both
offer only play, pause, stop and raise (TECHNICAL_DEBT.md).

`fastframe-now-playing` 0.17.0 (MIT, git tag on `crmne/fastframe`) puts both
behind one API. Research, 2026-11-08, reading the source at `v0.17.0`:
- VERIFIED: it does not depend on egui, so Cineo's egui 0.37 is unaffected.
  The crates of fastframe that use egui target 0.36.2 and are not usable.
- VERIFIED: it is not on crates.io; it is a git dependency pinned by tag.
- VERIFIED: on Linux it uses `mpris-server` 0.10.0 (MPL-2.0) and `zbus`
  5.19, the version `accesskit_unix` already brings in.
- VERIFIED: on Windows it uses `souvlaki` 0.8.3 (MIT), older than the 0.9.3
  Cineo used, and with it `windows` 0.44 next to the 0.62 in the tree. The
  controls get their own hidden window and message loop.
- VERIFIED: `NowPlaying` has no `Drop`. On Linux dropping it closes the
  channel that keeps the service running. On Windows the message loop never
  ends, so a dropped one would leak its thread and window.
- UNKNOWN: behavior against a real desktop, headset or Windows machine.

## Options considered
1. Keep the two backends: no new dependency, but two services to extend for
   position, seek and volume.
2. `fastframe-now-playing`: one backend and those features in its API, at
   the cost of a young git dependency, an older `souvlaki` and one MPL-2.0
   crate.

## Decision
`media_keys.rs` is the only media controls code in the desktop shell and
uses `fastframe-now-playing` on Linux and Windows.

- The controls start with the first loaded video.
- On Linux they are dropped when playback ends, so the MPRIS name still
  exists only during playback. On Windows they stay alive and show
  `Stopped`.
- The bus name stays `org.mpris.MediaPlayer2.cineo.instance<pid>`, and the
  desktop entry is `cineo`.
- The offered features did not change at first: play, pause, stop, raise
  and the title. Amended 2026-11-08: seek, position, length and volume are
  offered too. Next and previous are off and quit is not offered.

## Consequences
- `mpris.rs`, `smtc.rs`, the `souvlaki` dependency and the window handle
  passed to the media keys are gone. `zbus` stays for `keep_awake`.
- `cargo deny` allows `crmne/fastframe` as a git source and has a
  crate-scoped MPL-2.0 exception for `mpris-server` (LEGAL.md).
- All fastframe crates must share one tag, and its release notes must be
  read before moving it: its README says the APIs can change.
- Position, seek, volume and next episode can use the same API.

## Rejected alternatives
- Other fastframe crates in this change: `text` and `titlebar` need egui
  0.36; `power` and `instance` would each be a decision of their own.
- Dropping the controls on Windows too: it leaks a thread and a window on
  every playback.
