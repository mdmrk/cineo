#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

version=$1
target=$2
name="cineo-${version}-${target}"

exe=""
archive=tar.gz
if [[ "$target" == *windows* ]]; then
  exe=".exe"
  archive=zip
fi

mkdir -p "dist/$name"
cp "target/${target}/release/cineo-desktop${exe}" \
  README.md LICENSE crates/cineo-desktop/assets/fonts/*-OFL.txt "dist/$name/"

iconflow=$(cargo metadata --format-version 1 --locked \
  | jq -r '.packages[] | select(.name == "iconflow") | .manifest_path')
if command -v cygpath >/dev/null; then iconflow=$(cygpath -u "$iconflow"); fi
cp "$(dirname "$iconflow")/THIRD_PARTY_LICENSES_FONTS.md" "dist/$name/"

cd dist
if [[ "$archive" == zip ]]; then
  7z a "$name.zip" "$name" >/dev/null
else
  tar czf "$name.tar.gz" "$name"
fi
rm -rf "$name"
echo "dist/$name.$archive"
