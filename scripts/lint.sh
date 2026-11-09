#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export RUST_LOG=warn

step() { printf '\n==> %s\n' "$*"; }

have() { command -v "$1" >/dev/null 2>&1; }

need() {
  if have "$1"; then
    return 0
  fi
  if [[ -n "${CI:-}" ]]; then
    echo "$1 is required in CI" >&2
    exit 1
  fi
  echo "$1 not installed; skipped. Install: $2"
  return 1
}

step "taplo"
if need taplo "cargo install --locked taplo-cli"; then
  taplo lint
fi

step "shellcheck"
if need shellcheck "your package manager"; then
  git ls-files -z '*.sh' | xargs -0 shellcheck -x --severity=warning
fi

step "typos"
if need typos "cargo install --locked typos-cli"; then
  typos
fi

step "cargo-machete"
if need cargo-machete "cargo install --locked cargo-machete"; then
  cargo machete --with-metadata
fi

step "doc references"
scripts/check-docs.sh

step "lockfile"
cargo metadata --locked --format-version 1 >/dev/null

printf '\nLint passed.\n'
