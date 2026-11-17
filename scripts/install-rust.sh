#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

rustup toolchain install
if (($#)); then
  rustup target add "$@"
fi
