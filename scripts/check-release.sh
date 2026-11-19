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

check_assets() {
  local dir=${1:?usage: check-release.sh assets DIR} status=0 name
  [[ -d $dir ]] || die "$dir is not a directory"
  source scripts/libmpv-windows.env
  local expected=(
    cineo-x86_64-linux
    cineo-x86_64-linux.AppImage
    cineo-x86_64-linux.AppImage.zsync
    cineo-x86_64-windows.exe
    cineo-arm64-android.apk
    cineo-universal-macos.dmg
    "$LIBMPV_FILE"
    cineo-licenses.zip
    cineo.png
    install.sh
    install.ps1
  )
  for name in "${expected[@]}"; do
    [[ -f $dir/$name ]] || {
      echo "$dir has no $name" >&2
      status=1
    }
  done
  while IFS= read -r -d '' name; do
    name=$(basename "$name")
    printf '%s\n' "${expected[@]}" | grep -qxF -- "$name" || {
      echo "$dir has $name, which is not a release file" >&2
      status=1
    }
  done < <(find "$dir" -mindepth 1 -maxdepth 1 -print0)
  return "$status"
}

check_notes() {
  local version notes
  version=$(workspace_version)
  notes=docs/releases/v$version.md
  [[ -f $notes ]] || die "$notes is missing; write the notes of v$version with the version bump"
  grep -q '[^[:space:]]' "$notes" || die "$notes is empty"
}

check_key() {
  local key embedded
  [[ -f minisign.pub ]] || die "minisign.pub is missing"
  key=$(sed -n 2p minisign.pub)
  embedded=$(sed -n 's/^minisign_key=//p' scripts/install.sh)
  [[ -n $key && $key == "$embedded" ]] ||
    die "scripts/install.sh checks signatures with '$embedded', but minisign.pub holds '$key'"
}

case "${1:-}" in
  version) check_version "${2:-}" ;;
  assets) check_assets "${2:-}" ;;
  notes) check_notes ;;
  key) check_key ;;
  *) die "usage: check-release.sh version TAG | assets DIR | notes | key" ;;
esac
