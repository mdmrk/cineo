#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/common.sh

step "format"
scripts/fmt.sh --check

step "clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings

step "tests"
if have cargo-nextest; then
  cargo nextest run --workspace --locked
elif [[ -n "${CI:-}" ]]; then
  echo "cargo-nextest is required in CI" >&2
  exit 1
else
  cargo test --workspace --locked
fi

if [[ "${1:-}" != "--quick" ]]; then
  step "lint"
  scripts/lint.sh

  step "release checks"
  scripts/test-check-release.sh
  scripts/check-release.sh notes
  scripts/check-release.sh key
  if need minisign "your package manager"; then
    scripts/test-sign-release.sh
    scripts/test-install.sh
  fi

  step "cargo-deny"
  if need cargo-deny "cargo install --locked cargo-deny"; then
    cargo deny --config .config/deny.toml check
  fi
fi

printf '\nAll checks passed.\n'
