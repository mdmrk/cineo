#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

die() {
  echo "$*" >&2
  exit 1
}

workspace_version() {
  sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml
}

check_version() {
  local tag=${1:?usage: check-release.sh version TAG} version
  version=$(workspace_version)
  [[ -n $version ]] || die "Cargo.toml has no [workspace.package] version"
  [[ ${tag#v} == "$version" ]] ||
    die "the tag $tag names ${tag#v}, but Cargo.toml says $version; bump the version or retag"
}

case "${1:-}" in
  version) check_version "${2:-}" ;;
  *) die "usage: check-release.sh version TAG" ;;
esac
