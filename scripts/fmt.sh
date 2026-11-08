#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export RUST_LOG=warn

step() { printf '\n==> %s\n' "$*"; }

have() { command -v "$1" >/dev/null 2>&1; }

mode_check=0
[[ "${1:-}" == "--check" ]] && mode_check=1

shell_files() { git ls-files -z '*.sh'; }

step "rustfmt"
if ((mode_check)); then cargo fmt --all -- --check; else cargo fmt --all; fi

step "taplo"
if have taplo; then
  if ((mode_check)); then taplo fmt --check; else taplo fmt; fi
else
  echo "taplo not installed; skipped. Install: cargo install --locked taplo-cli"
fi

step "shfmt"
if have shfmt; then
  if ((mode_check)); then
    shell_files | xargs -0 shfmt -i 2 -ci -d
  else
    shell_files | xargs -0 shfmt -i 2 -ci -w
  fi
else
  echo "shfmt not installed; skipped. Install: https://github.com/mvdan/sh/releases"
fi

printf '\nFormatting done.\n'
