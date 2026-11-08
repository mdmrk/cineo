# 0032. Link handover, file replacement, system sleep and panic log through fastframe

- Status: Accepted
- Date: 2026-11-08
- Supersedes: the handover channel of 0018 and 0027

## Context
The owner asked to use fastframe wherever it fits (2026-11-08), after
ADR-0031 adopted `fastframe-now-playing`. Each crate of v0.17.0 was read at
that tag.

- VERIFIED: `fastframe-instance` and `fastframe-file` do not depend on egui.
- VERIFIED: `fastframe-instance` holds an OS file lock, and a socket of mode
  0600 in a directory of mode 0700 on Unix. On Windows it uses a loopback
  port and requires a 64-digit random token kept in the slot's directory.
- VERIFIED: `Slot::claim` makes the slot's directory mode 0700, so a slot
  directly in a directory given with `--data-dir` would change that
  directory's mode.
- VERIFIED: `fastframe_file::write` writes a temporary file, syncs it, moves
  it over the target and syncs the directory.
- UNKNOWN: the loopback channel on a real Windows machine.

## Options considered
1. Keep the hand-written socket, named pipe and partial-file code.
2. Use the fastframe crates where they keep what Cineo does today.

## Decision
- `instance.rs` uses `Slot::at(<data dir>/instance, "cineo")`. A launch with
  a link sends it and exits if the running copy answers. The first copy
  claims the slot when its window starts and keeps the lock until it exits.
- A later launch without a link claims with `ping`, which is never treated
  as a link, then keeps running as it did before (TECHNICAL_DEBT.md).
- `libmpv.rs`, the Linux `register.rs` entry and the torrent blocklist in
  `cineo-stream` use `fastframe_file::write`.
- While a video plays, `keep_awake.rs` also holds a
  `fastframe_power::KeepAwake` against idle system sleep. The existing
  ScreenSaver inhibition and `keepawake` stay, because only they keep the
  screen on.
- `panic_log.rs` records each panic in `<data dir>/panic.log` with
  `fastframe_log::log_panics` (time, version, thread and location, never the
  message), then runs the default hook so a terminal still shows it.

## Consequences
- The Windows named pipe is replaced by a loopback port with a token. A
  local process that can read the user's profile can send a link, which
  stays untrusted input (SECURITY.md).
- A link may be up to 16 KiB in total, including the prefix, instead of
  8 KiB.
- A `cineo.sock` left by an earlier version stays in the data directory,
  unused.
- A failed sync of the cache directory now stops the streaming engine, as a
  failed blocklist write already did. `cineo-stream` brings `fastframe-file`
  into the Android graph too (INFERRED to build there; not tried).

## Rejected alternatives
Other crates, as of v0.17.0:
- Replacing `keep_awake.rs` with `power`: `KeepAwake` only blocks suspend
  (`PowerRequestSystemRequired`, the portal's Suspend flag, a logind idle
  lock), and the screen must stay on during a video.
- `dirs`: it offers config, state and cache folders. Cineo's database is in
  the data folder, so moving would strand existing libraries.
- `file` in `cineo-player` subtitles: the files are new and uniquely named,
  so atomic replacement adds nothing, and they are written on the UI thread,
  where a sync would stall the window.
- `text`, `fonts`, `emoji`, `icons`, `theme`, `titlebar`, `scroll`, `shell`,
  `graphics`: they need egui 0.36 or its `eframe`; Cineo is on 0.37.
- `i18n`: replacing rust-i18n (ADR-0021) means moving 336 call sites and
  every locale to PO files, for no user-visible gain.
- `update`: needs Ed25519-signed releases and a change to the release
  process.
- `tray`, `dock`, `audio`, `notifications`, `autostart`: Cineo has no
  equivalent to replace; each would be a new feature.
- The `log` crate's logger: Cineo logs through `tracing`, and the crate has
  no `tracing` subscriber.
