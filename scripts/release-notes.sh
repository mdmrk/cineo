#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

tag=${1:?usage: release-notes.sh TAG [PREVIOUS_TAG]}
previous=${2:-}
notes=docs/releases/$tag.md
[[ -f $notes ]] || {
  echo "$notes is missing; write the notes before tagging" >&2
  exit 1
}
repo=https://github.com/${GITHUB_REPOSITORY:-mdmrk/cineo}
download=$repo/releases/download/$tag

printf '**Download:** [Linux AppImage](%s) · [Linux](%s) · [Windows](%s) · [macOS](%s) · [Android](%s)\n\n' \
  "$download/cineo-x86_64-linux.AppImage" "$download/cineo-x86_64-linux" "$download/cineo-x86_64-windows.exe" \
  "$download/cineo-universal-macos.dmg" "$download/cineo-arm64-android.apk"
cat "$notes"
if [[ -n $previous ]]; then
  printf '\n**Full changelog**: %s/compare/%s...%s\n' "$repo" "$previous" "$tag"
fi
