#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/common.sh
usage="usage: release.sh version TAG | assets DIR | notes | key | body TAG [PREVIOUS_TAG] | sign DIR TAG | tool DIR"
minisign_url=${CINEO_MINISIGN_URL:-https://github.com/jedisct1/minisign/releases/download/0.12/minisign-0.12-linux.tar.gz}
minisign_sha256=9a599b48ba6eb7b1e80f12f36b94ceca7c00b7a5173c95c3efc88d9822957e73
check_version() {
  local tag=${1:?$usage} version
  version=$(workspace_version)
  [[ -n $version ]] || die "Cargo.toml has no [workspace.package] version"
  [[ ${tag#v} == "$version" ]] ||
    die "the tag $tag names ${tag#v}, but Cargo.toml says $version; bump the version or retag"
}
check_assets() {
  local dir=${1:?$usage} status=0 name
  [[ -d $dir ]] || die "$dir is not a directory"
  source scripts/libmpv-windows.env
  local expected=(
    cineo-x86_64-linux
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
  embedded=$(sed -n 's/^pub const PUBLIC_KEY: &str = "\(.*\)";$/\1/p' crates/cineo-desktop/src/self_update.rs)
  [[ $key == "$embedded" ]] ||
    die "the Windows updater checks signatures with '$embedded', but minisign.pub holds '$key'"
}
body() {
  local tag=${1:?$usage} previous=${2:-} notes repo download
  notes=docs/releases/$tag.md
  [[ -f $notes ]] || die "$notes is missing; write the notes before tagging"
  repo=https://github.com/${GITHUB_REPOSITORY:-mdmrk/cineo}
  download=$repo/releases/download/$tag
  printf '**Download:** [Linux](%s) · [Windows](%s) · [macOS](%s) · [Android](%s)\n\n' \
    "$download/cineo-x86_64-linux" "$download/cineo-x86_64-windows.exe" \
    "$download/cineo-universal-macos.dmg" "$download/cineo-arm64-android.apk"
  cat "$notes"
  [[ -z $previous ]] || printf '\n**Full changelog**: %s/compare/%s...%s\n' "$repo" "$previous" "$tag"
}
sign() {
  local dir=${1:?$usage} tag=${2:?$usage} sums
  sums=$dir/SHA256SUMS
  [[ -f $sums ]] || die "$sums is missing"
  [[ -n ${CINEO_MINISIGN_KEY:-} ]] || die "CINEO_MINISIGN_KEY is not set; refusing to publish unsigned checksums"
  [[ $(sed -n 1p <<<"$CINEO_MINISIGN_KEY") == "untrusted comment:"* && -n $(sed -n 2p <<<"$CINEO_MINISIGN_KEY") ]] ||
    die "CINEO_MINISIGN_KEY must hold the whole secret key file: its 'untrusted comment:' line and the key line below it"
  key=$(mktemp)
  trap 'rm -f "$key"' EXIT
  chmod 600 "$key"
  printf '%s\n' "$CINEO_MINISIGN_KEY" >"$key"
  minisign -S -s "$key" -m "$sums" -x "$sums.minisig" -t "cineo $tag SHA256SUMS" >/dev/null
  minisign -V -q -p minisign.pub -m "$sums" -x "$sums.minisig"
}
tool() {
  local dir=${1:?$usage}
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
  fetch "$work/minisign.tar.gz" "$minisign_sha256" "$minisign_url"
  tar -xzf "$work/minisign.tar.gz" -C "$work" minisign-linux/x86_64/minisign 2>/dev/null
  mkdir -p "$dir"
  install -m 755 "$work/minisign-linux/x86_64/minisign" "$dir/minisign"
}
command=${1:-}
shift || true
case "$command" in
  version) check_version "$@" ;;
  assets) check_assets "$@" ;;
  notes) check_notes ;;
  key) check_key ;;
  body) body "$@" ;;
  sign) sign "$@" ;;
  tool) tool "$@" ;;
  *) die "$usage" ;;
esac
