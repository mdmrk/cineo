# 0018. Deep links: typed routes in core, single-instance handover, opt-in scheme registration

- Status: Accepted
- Date: 2026-10-08

## Context
Stremio parity (ADR-0010) includes `stremio://` deep links: addon install
links and page links (board, discover, library, search, detail), as
documented in the SDK's `deep-links.md` (VERIFIED-DOC, read 2026-10-08).
Cineo also accepts the same forms under `cineo://`. The owner approved the
plan on 2026-10-08, with registration opt-in from Settings.

A link reaches a desktop app as a command-line argument of a new process
(Linux `.desktop` `%u`, Windows `HKCU\Software\Classes\<scheme>`). When
Cineo is already running, that new process must hand the link to the
running window instead of opening a second one. macOS delivers links as
Apple Events to an app bundle that declares `CFBundleURLTypes`; Cineo has no
bundle until packaging (M7). VERIFIED for Linux and Windows (platform docs);
INFERRED for macOS (not tried).

Research (2026-10-08): `interprocess` 2.4.4 (0BSD OR Apache-2.0) offers
cross-platform local sockets. `single-instance` 0.3.3 was last released in
2021. tokio, already a dependency with the `net` feature, has named pipes
on Windows, and the standard library has Unix sockets.

## Options considered
1. **Parse in core; hand over through a Unix socket or a named pipe built
   on std and tokio; registration written by the app on request.** No new
   dependency.
2. Use `interprocess` for the handover. One API for both platforms, but a
   new dependency for about 100 lines of code.
3. Register the schemes automatically on first start. Convenient, but it
   silently takes `stremio://` links away from an installed Stremio.

## Decision
- `cineo-core::app::parse_link` turns a link into a typed `Route`. Anything
  outside the allowlist is rejected (`LinkError`) and the shell shows
  `Notice::InvalidLink`. Install links map only to `https://` and set
  `State::link_prompt`; nothing is installed before `AcceptLink`. Page links
  only navigate. `autoPlay` is ignored.
- `cineo-desktop <link>` opens a link. The startup link waits until the
  installed addons have loaded.
- Single instance, for links only: the running app listens on
  `<data dir>/cineo.sock` (mode 0600) on Unix, or on the named pipe
  `\\.\pipe\cineo-<hash of the data dir>` (remote clients rejected) on
  Windows. A launch with a link first tries to hand it over and exits if
  that works. One line of at most 8 KiB is read per connection.
- Registration is opt-in from Settings → Interface. Linux writes a per-user
  `cineo-links.desktop` and runs `xdg-mime default`; Windows writes the
  `HKCU` keys with `reg.exe`. Nothing needs administrator rights.

## Consequences
- Platform code (`instance.rs`, `register.rs`) lives in the desktop shell
  (ARCHITECTURE.md §Platform abstraction).
- Anything that can connect to the socket or pipe as the same user can make
  the window open a link. Links are untrusted anyway (SECURITY.md).
- macOS links and plain second launches without a link are not handled
  (TECHNICAL_DEBT.md).

## Rejected alternatives
- `interprocess` (option 2): saves little code and adds a dependency.
- Automatic registration (option 3): it changes another app's links without
  asking.
