#!/usr/bin/env bash
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
else
  cargo test --workspace --locked
fi

if [[ "${1:-}" != "--quick" ]]; then
  step "docs"
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

  step "cargo-deny"
  if command -v cargo-deny >/dev/null 2>&1; then
    cargo deny --all-features --config .config/deny.toml check
  else
    echo "cargo-deny not installed; skipped (CI runs it). Install: cargo install --locked cargo-deny"
  fi
fi

printf '\nAll checks passed.\n'
