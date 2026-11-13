# 0036. A pinned stable toolchain, bumped by hand

- Status: Accepted
- Date: 2026-11-13
- Supersedes: 0025

## Context
ADR-0025 removed `rust-toolchain.toml` so that CI always installs the
latest `stable`. `Swatinem/rust-cache` hashes the `rustc` version into
both its cache key and its fallback key (VERIFIED in its `src/config.ts`,
v2.9.3), so every new stable release, about every six weeks, makes every
job on `main` start from a cold cache at once. Warm runs on `main` take 3
to 5 minutes and several runs took 9 to 30 (VERIFIED on the run history up
to 2026-11-13); how many of those a toolchain change made cold is UNKNOWN,
because the job logs were not read. New lints also break CI on a day nobody changed the code, as
clippy 1.100 did on 2026-11-12. The owner asked to make CI as fast as
possible (owner decision 2026-11-13).

## Options considered
1. Keep the latest `stable` (ADR-0025): no bumps to make, but a cold cache
   and possibly new lints whenever Rust releases.
2. Pin the stable version in `rust-toolchain.toml` and bump it by hand: the
   cache stays warm until the bump, and the bump is a commit that fixes the
   new lints in the same change.

## Decision
- `rust-toolchain.toml` pins a stable release (`channel = "X.Y.Z"`) with
  the `minimal` profile, rustfmt and clippy.
- CI installs the pinned version with `scripts/install-rust.sh`, which
  reads it from that file and also makes it rustup's default, so a command
  run outside the checkout uses the same version.
- No `rust-version` in any `Cargo.toml`: Cineo is an application, not a
  library, so there is still no MSRV.
- A bump is its own commit, `build: use Rust X.Y.Z`, with the lint fixes
  it needs.

## Consequences
CI's Rust cache is cold only after a bump; a `Cargo.lock` change still
restores the previous cache through the fallback key. A contributor with rustup gets the pinned version automatically. One with a
distro-packaged Rust must use the pinned version or newer. Someone has to
bump the pin; nothing reminds us when a new stable is out. The F-Droid
recipe must install the version pinned at the tag it builds, with the
Android target: with another version, rustup downloads the pinned one
without that target and the build fails (INFERRED from rustup's automatic
install; not tried). `docs/DEVELOPMENT.md` and `docs/TECHNICAL_DEBT.md` change.

## Rejected alternatives
Keeping the latest `stable`: it costs a cold CI cache with every Rust
release and lets a release break CI without a code change.
