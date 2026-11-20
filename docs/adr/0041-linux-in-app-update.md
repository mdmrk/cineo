# 0041. In-app update on Linux

- Status: Accepted
- Date: 2026-11-19
- Amends: 0040 (the same update, now also on Linux)

## Context
ADR-0040 lets Cineo update itself on Windows; Linux keeps the "vX.Y.Z
available" notice and users run `install.sh` again. The owner asked on
2026-11-19 for the same update on Linux.

Facts (2026-11-19):
- `install.sh` puts the executable at `~/.local/bin/cineo-desktop` (or
  `$XDG_BIN_HOME/cineo-desktop`), a folder the user can write to, and the
  license texts in `$XDG_DATA_HOME/doc/cineo`, not beside the executable,
  and only when `unzip` is installed (VERIFIED: source).
- Every release carries `cineo-x86_64-linux` with a line in the signed
  `SHA256SUMS` (VERIFIED: `scripts/release.sh assets`, `install.sh`).
- The Linux build uses the system libmpv, so the executable is the whole
  update (VERIFIED: ADR-0014, ADR-0027).
- On Linux a running executable can be renamed and another file renamed
  into its place; the running process keeps its open file (VERIFIED:
  `a_running_executable_can_be_swapped` on Linux).
- `std::fs::write` creates files with mode 0644 minus the umask, so a
  written executable cannot run (VERIFIED: std docs).
- `minisign-verify` and aws-lc-rs are already in the Linux build
  (VERIFIED: `crates/cineo-desktop/Cargo.toml`, `cargo tree`).

## Options considered
1. **Reuse ADR-0040's update on Linux** with the Linux asset and name.
   Small change; same verification and tests.
2. **Run `install.sh` from the app.** It verifies the signature when
   minisign is installed, but spawns a shell and curl and also rewrites the
   desktop entry.

## Decision
Option 1.

- **Core.** `State::self_update` becomes `Option<UpdateTarget>`
  (`Windows` or `Linux`); the shell sets it only when this copy can
  update itself. `UpdateRequest` names the target, whose release asset
  is `cineo-x86_64-windows.exe` or `cineo-x86_64-linux`.
- **When.** Linux: the running executable is named `cineo-desktop` and its
  folder is writable (an `install.sh` install, or a copy the user put
  there). Amended 2026-11-20: the executable is now `cineo`; `cineo` and
  the older `cineo-desktop` both update themselves, and `install.sh`
  installs `~/.local/bin/cineo` and deletes `cineo-desktop`. A system install in `/usr/bin` keeps the notice. macOS keeps the
  notice.
- **Download and verify.** As ADR-0040, with `cineo-x86_64-linux` as the
  executable. The license zip is not downloaded on Linux.
- **Swap.** As ADR-0040, with `cineo-desktop.new` created with mode 0755
  (minus the umask) and `cineo-desktop.old` deleted on the next start.
- **Restart.** As ADR-0040.

## Consequences
- Linux users of `install.sh` update with two clicks.
- The license texts `install.sh` wrote stay at the version installed;
  running `install.sh` again refreshes them (`docs/TECHNICAL_DEBT.md`).
- A development build at `target/*/cineo-desktop` offers the update too,
  as on Windows.
- Tests: the core test names the target's asset; the swap keeps the new
  executable runnable; on Linux, renaming a running executable. A real
  update from one release to the next is UNKNOWN until done by hand.
- Docs: `docs/COMPATIBILITY.md`, `docs/SECURITY.md`,
  `docs/ARCHITECTURE.md`, `docs/DEVELOPMENT.md#dependencies`, README.

## Rejected alternatives
- **`install.sh` from the app (2):** a shell and curl from the app, and the
  signature check depends on minisign being installed.
