#!/usr/bin/env bash
step() { printf '\n==> %s\n' "$*"; }
die() {
  echo "$*" >&2
  exit 1
}
have() { command -v "$1" >/dev/null 2>&1; }
need() {
  if have "$1"; then
    return 0
  fi
  [[ -z "${CI:-}" ]] || die "$1 is required in CI"
  echo "$1 not installed; skipped. Install: $2"
  return 1
}
workspace_version() {
  sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "${1:-Cargo.toml}"
}
