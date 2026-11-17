#!/usr/bin/env bash
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
