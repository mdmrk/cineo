#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export RUST_LOG=warn

source scripts/common.sh

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

printf '\nLint passed.\n'
