#!/usr/bin/env bash
# The local definition of "done": the same gates CI runs.
# Usage: scripts/check.sh [--quick]   (--quick skips docs and cargo-deny)
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

step "rustfmt"
cargo fmt --all -- --check

step "clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings

step "tests"
if command -v cargo-nextest >/dev/null 2>&1; then
  cargo nextest run --workspace --locked
  cargo test --workspace --locked --doc
else
  cargo test --workspace --locked
fi

if [[ "${1:-}" != "--quick" ]]; then
  step "docs"
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

  step "cargo-deny"
  if command -v cargo-deny >/dev/null 2>&1; then
    cargo deny check
  else
    echo "cargo-deny not installed; skipped (CI runs it). Install: cargo install --locked cargo-deny"
  fi
fi

printf '\nAll checks passed.\n'
