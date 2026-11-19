#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/common.sh
step "format"
scripts/fmt.sh --check
step "clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings
step "tests"
if need cargo-nextest "cargo install --locked cargo-nextest"; then
  cargo nextest run --workspace --locked
else
  cargo test --workspace --locked
fi
[[ "${1:-}" == "--quick" ]] && exit 0
export RUST_LOG=warn
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
step "release scripts"
scripts/test-release.sh
scripts/release.sh notes
scripts/release.sh key
step "cargo-deny"
if need cargo-deny "cargo install --locked cargo-deny"; then
  cargo deny --config .config/deny.toml check
fi
printf '\nAll checks passed.\n'
