# 0027. Windows x86_64 support

- Status: Accepted (media keys: superseded by 0031)
- Date: 2026-11-03
- Supersedes: the Windows parts of 0022

## Context
ADR-0022 dropped Windows and macOS on 2026-10-13 because nobody ran them.
On 2026-11-03 the owner asked for Windows again, with full desktop
integration, a single executable that carries libmpv, and macOS still
unsupported.

What existed before 0022 and was removed by `37d90bc`: the named-pipe
handover and `reg.exe` scheme registration (ADR-0018), the bundled
`libmpv-2.dll` (ADR-0014, amended 2026-10-12), `seek_read`/`seek_write`
torrent storage and the `install.ps1` installer. None of it was ever run on
a Windows machine (ADR-0022). Since then, Linux-only code was added: media
keys over MPRIS and a screen-saver inhibitor, both over D-Bus with `zbus`.

Research (2026-11-03):
- `souvlaki` 0.9.3 (MIT, crates.io 2026-10-27) wraps the Windows System
  Media Transport Controls (SMTC) and needs the window's HWND. With default
  features off it depends only on `windows` 0.62 on Windows (VERIFIED:
  crates.io metadata and source).
- `keepawake` 0.6.1 (MIT, 2026-08-20) wraps `SetThreadExecutionState`; the
  state belongs to the calling thread and is undone on drop (VERIFIED:
  source).
- `windows` 0.62.2 is already in the Windows graph through
  `accesskit_windows` and librqbit (VERIFIED: `cargo tree --target
  x86_64-pc-windows-msvc`).
- zhongfly/mpv-winbuild `2026-11-02-a687796d9d` ships
  `mpv-dev-lgpl-x86_64-20261102-git-a687796d9d.7z`, SHA-256
  `8b61f897…d2e9` (VERIFIED: downloaded and hashed), built from mpv
  `a687796d9d0e`.

## Options considered
1. **Restore the pre-0022 Windows code, and add SMTC and keep-awake through
   `souvlaki` and `keepawake`.** No `unsafe` in the desktop shell; two small
   MIT dependencies, Windows-only.
2. **Same, but call `windows` directly.** No new crates, but
   `SetThreadExecutionState` and `ISystemMediaTransportControlsInterop::GetForWindow`
   are `unsafe`, and `unsafe` is forbidden outside the player's FFI
   (ADR-0014).
3. **Windows without media keys and keep-awake.** Less code; the screen
   sleeps during playback.

## Decision
Option 1. Cineo supports Linux x86_64 and Windows x86_64. macOS stays
unsupported (ADR-0022).

- **libmpv.** Windows release builds use the `bundled-libmpv` feature of
  `cineo-desktop`, which embeds the pinned LGPL `libmpv-2.dll`
  (`scripts/libmpv-windows.env`) and writes it to
  `<cache dir>/libmpv/<version>/` on start if it is missing or has another
  size. Lookup order: beside the executable, then the extracted copy, then
  the system names. A DLL beside the executable replaces it (LGPL-2.1 §6).
  Linux keeps using the system libmpv. `hwdec` is `auto-safe` on Windows
  and `vaapi,auto-safe` on Linux.
- **Single instance.** The named pipe
  `\\.\pipe\cineo-<hash of the data dir>`, first instance only, remote
  clients rejected (ADR-0018).
- **Scheme registration.** `reg.exe` writes `HKCU\Software\Classes\<scheme>`
  for `stremio` and `cineo`, without a console window.
- **Media keys.** SMTC through `souvlaki`, created on the UI thread with
  the window's HWND; Play, Pause, Stop and Raise map to the same `Remote`
  values as MPRIS.
- **Keep awake.** `keepawake` with display and system required, held by
  the existing keep-awake thread.
- **Files.** Private directories and files keep mode 0700/0600 on Unix;
  Windows uses the default ACL of the user's profile.
- **Release builds** use `windows_subsystem = "windows"` (no console).
- **CI** runs clippy and the tests on `windows-latest` too, and builds
  `cineo-x86_64-windows.exe` for releases. `cargo deny` checks the Linux and
  Windows graphs.

## Consequences
- Windows behavior is UNKNOWN until someone runs it; COMPATIBILITY.md says
  so per feature.
- Releases carry an LGPL binary again: `cineo-licenses.zip` gets
  `LGPL-2.1.txt` and `libmpv.md` with the source links.
- `zbus` becomes a Linux-only dependency; `souvlaki` and `keepawake` are
  Windows-only.
- Platform code stays in the desktop shell, `cineo-stream` and
  `cineo-store` behind `cfg`. `std::os::unix` now needs `cfg(unix)`.

## Rejected alternatives
- Option 2: it puts `unsafe` in the desktop shell to save two small crates.
- Option 3: the owner asked for full integration.
