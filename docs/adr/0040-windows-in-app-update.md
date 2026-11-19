# 0040. In-app update on Windows

- Status: Accepted; amended by 0041 (Linux)
- Date: 2026-11-19

## Context
Cineo checks GitHub's latest release once per start and, when it is newer,
shows "vX.Y.Z available" in the sidebar, which opens the release page
(`release::parse_release`, `Effect::CheckForUpdate`; VERIFIED: source and
tests). Updating means running `install.ps1` again. The owner asked on
2026-11-19 for Cineo to update itself on Windows; other platforms keep the
notice.

Facts (2026-11-19):
- `install.ps1` puts the executable at
  `%LOCALAPPDATA%\Programs\Cineo\cineo-desktop.exe` and the license texts in
  `licenses\` beside it (VERIFIED: source). A portable copy runs from any
  folder (`dirs::is_portable`).
- Every release and the prerelease carry `cineo-x86_64-windows.exe`,
  `cineo-licenses.zip`, `SHA256SUMS` and `SHA256SUMS.minisig`, signed in
  CI with the key in `minisign.pub`; the trusted comment is
  `cineo <tag> SHA256SUMS` (`scripts/release.sh sign`; VERIFIED on the
  published prerelease).
- `cineo-net` caps a response at 8 MiB and 20 s by default (`NetPolicy`);
  the executable is about 69 MB (VERIFIED: prerelease).
- The Windows build already links aws-lc-rs through librqbit, which has
  SHA-256 (VERIFIED: `cargo tree -i aws-lc-rs --target
  x86_64-pc-windows-msvc`). minisign signatures are Ed25519 over a
  BLAKE2b-512 hash, which aws-lc-rs does not offer; `minisign-verify` 0.3.0
  (MIT, no dependencies, by minisign's author) verifies them (VERIFIED:
  crates.io).
- Windows refuses to overwrite or delete a running executable but allows
  renaming it (INFERRED; to be confirmed by a test on the Windows runner).
- The libmpv DLL inside the executable is written to a cache folder keyed by
  its checksum (ADR-0027), so a new executable brings its own.

## Options considered
1. **Cineo downloads, verifies and swaps its own executable** (rename the
   running one aside, move the new one in, delete the old one on the next
   start). No new process to ship; small code in the desktop shell.
2. **A separate updater executable or MSI** (spotifast's model). Handles
   locked files and rollback through Windows Installer, but adds a second
   artifact, WiX and signing work.
3. **The `self-replace` crate** for the swap. Same idea as 1, plus an
   Apache-2.0 dependency with `windows-sys`, for what `std::fs::rename`
   does.
4. **Run `install.ps1` from the app.** Reuses tested code, but spawns
   PowerShell from the app, cannot verify the signature (it does not) and
   fails while Cineo runs.

## Decision
Option 1, on Windows only.

- **When.** Only when the running executable's folder is writable and the
  executable is named `cineo-desktop.exe` (an `install.ps1` install or a
  portable copy). Otherwise the sidebar keeps opening the release page.
  Nothing downloads until the user clicks "Update to vX.Y.Z".
- **Core.** `cineo-core` gains the update's state (available, downloading,
  ready, failed with a typed reason) and an effect that names the
  version and the release's asset URLs; it does no IO and returns typed
  messages (ADR-0001, ADR-0017). The shell tells it whether this copy can
  update itself.
- **Download.** Through `cineo-net` with an update policy: public HTTPS
  only, up to 256 MiB and 10 minutes (ADR-0006). `SHA256SUMS`, its
  signature, the executable and, if `licenses\` exists, `cineo-licenses.zip`.
- **Verify before touching anything.** The signature must verify with the
  public key built into Cineo (the one in `minisign.pub`; a lint keeps them
  equal) and its trusted comment must be exactly
  `cineo vX.Y.Z SHA256SUMS` for the version offered, so an older signed
  release cannot be replayed. Each file's SHA-256 must match its line.
  Any failure leaves the installed copy as it was.
- **Swap.** Write `cineo-desktop.exe.new` beside the executable, rename the
  running `cineo-desktop.exe` to `cineo-desktop.exe.old`, rename the new
  one into place; if the second rename fails, rename the old one back.
  `licenses\` is replaced from the zip after the swap. On every start,
  Cineo deletes a leftover `cineo-desktop.exe.old`.
- **Restart.** The sidebar then offers "Restart to update". Cineo starts
  the new executable with its own arguments plus `--after-update` and
  quits; the new one waits while the old one still answers the
  single-instance ping (at most 10 s, ADR-0032), so it does not hand itself
  over to the closing one. (Amended 2026-11-19, before any code: waiting
  for a process id would need `unsafe` Windows calls, which the desktop
  shell forbids; the ping already exists and has no side effects.)
- **Dependency.** `minisign-verify` 0.3.0; SHA-256 from aws-lc-rs, already
  in the Windows build.

## Consequences
- Windows users update with two clicks; a broken download or a bad
  signature changes nothing.
- A leaked signing key would let someone ship an update; key rotation needs
  a release that carries both the old and new key, which this ADR does not
  build. The key stays in the `release-signing` environment only.
- Releases must keep the asset names and the trusted comment format;
  `scripts/release.sh` and `scripts/test-release.sh` already
  check them.
- Tests: core state transitions; signature and hash checks with a test key
  (good, bad signature, other version's comment, wrong hash); the swap and
  rollback in a temporary folder; on the Windows runner, renaming a running
  executable. A real update from one release to the next is UNKNOWN until
  done by hand.
- Docs: `docs/COMPATIBILITY.md` (Update notice), `docs/SECURITY.md`,
  `docs/ARCHITECTURE.md`, `docs/DEVELOPMENT.md#dependencies`,
  `docs/LEGAL.md` (no new license: MIT).

## Rejected alternatives
- **Updater executable or MSI (2):** more artifacts and tooling than one
  self-contained executable needs.
- **`self-replace` (3):** a dependency for two renames.
- **`install.ps1` from the app (4):** no signature check and fails while
  Cineo runs.
