#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

channel=$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)
if [[ -z $channel ]]; then
  echo "rust-toolchain.toml names no channel" >&2
  exit 1
fi
args=(--profile minimal --component rustfmt,clippy)
for target in "$@"; do
  args+=(--target "$target")
done
rustup toolchain install "$channel" "${args[@]}"
rustup default "$channel"
