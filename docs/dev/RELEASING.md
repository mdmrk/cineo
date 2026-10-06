# Releasing

There are no releases yet. The pipeline exists so the first release is
boring.

- **Versioning:** SemVer. `0.x` until v1.0, and anything may change in
  `0.x`. The workspace version in the root `Cargo.toml` is the single source.
- **Changelog:** [Keep a Changelog](https://keepachangelog.com) format in
  `CHANGELOG.md`. Every user-visible change adds a line under *Unreleased*
  in the same PR.
- **Tags:** `vMAJOR.MINOR.PATCH`, with an optional pre-release suffix
  (`v0.1.0-alpha.1`). Tags containing `-` become GitHub pre-releases.

## Steps

1. On `main` with green CI: move the *Unreleased* entries to a new version
   section and bump `version` in `Cargo.toml`.
2. Commit with `chore(release): vX.Y.Z`.
3. Tag `vX.Y.Z` and push the tag.
4. `.github/workflows/release.yml`:
   - tests and builds `cineo` for Linux x86_64, Windows x86_64 and macOS
     arm64,
   - packages each build with the README and licenses,
   - attests build provenance,
   - writes `SHA256SUMS`,
   - creates a **draft** release.
5. Review the draft (download, check a checksum, run
   `gh attestation verify <file> --repo <owner>/cineo`), then publish.

Note: the workflow builds the `cineo-cli` package, which arrives in M1. Do
not tag before then.

## Not yet decided (M7)

- Code signing (Windows Authenticode, macOS notarization) and installer
  formats.
- Bundling mpv (license review: LEGAL.md).
- Reproducible-build verification beyond `--locked` + a pinned toolchain.
