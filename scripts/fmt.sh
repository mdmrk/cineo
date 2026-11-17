#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export RUST_LOG=warn

source scripts/common.sh

mode_check=0
[[ "${1:-}" == "--check" ]] && mode_check=1

shell_files() { git ls-files -z '*.sh'; }

step "rustfmt"
if ((mode_check)); then cargo fmt --all -- --check; else cargo fmt --all; fi

step "taplo"
if need taplo "cargo install --locked taplo-cli"; then
  if ((mode_check)); then taplo fmt --check; else taplo fmt; fi
fi

step "shfmt"
if need shfmt "https://github.com/mvdan/sh/releases"; then
  if ((mode_check)); then
    shell_files | xargs -0 shfmt -i 2 -ci -d
  else
    shell_files | xargs -0 shfmt -i 2 -ci -w
  fi
fi

printf '\nFormatting done.\n'
