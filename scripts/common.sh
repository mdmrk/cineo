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
crate_dir() {
  local manifest
  manifest=$(cargo metadata --format-version 1 --locked |
    jq -r --arg name "$1" 'first(.packages[] | select(.name == $name) | .manifest_path)')
  [[ -n $manifest ]] || die "cargo metadata has no crate $1"
  dirname "$manifest"
}
sha256_matches() {
  if have sha256sum; then
    echo "$1  $2" | sha256sum -c --status - 2>/dev/null
  else
    echo "$1  $2" | shasum -a 256 -c --status - 2>/dev/null
  fi
}
fetch() {
  local out=$1 sha=$2 url retry=(--retry 5)
  shift 2
  sha256_matches "$sha" "$out" && return 0
  (($# > 1)) || retry+=(--retry-all-errors)
  for url in "$@"; do
    curl -fsSL "${retry[@]}" -o "$out" "$url" && sha256_matches "$sha" "$out" && return 0
  done
  rm -f "$out"
  die "no download of $(basename "$out") matches its pinned SHA-256 $sha: $*"
}
