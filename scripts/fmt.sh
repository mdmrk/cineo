#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export RUST_LOG=warn
source scripts/common.sh
check=()
shfmt_mode=-w
if [[ "${1:-}" == "--check" ]]; then
  check=(--check)
  shfmt_mode=-d
fi
step "rustfmt"
cargo fmt --all -- "${check[@]}"
step "taplo"
if need taplo "cargo install --locked taplo-cli"; then
  taplo fmt "${check[@]}"
fi
step "shfmt"
if need shfmt "https://github.com/mvdan/sh/releases"; then
  git ls-files -z '*.sh' | xargs -0 shfmt -i 2 -ci "$shfmt_mode"
fi
printf '\nFormatting done.\n'
