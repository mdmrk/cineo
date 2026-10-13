#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
rm -rf dist
mkdir -p dist/cineo-licenses

cp LICENSE crates/cineo-desktop/assets/fonts/*-OFL.txt dist/cineo-licenses/
iconflow=$(cargo metadata --format-version 1 --locked \
  | jq -r '.packages[] | select(.name == "iconflow") | .manifest_path')
cp "$(dirname "$iconflow")/THIRD_PARTY_LICENSES_FONTS.md" dist/cineo-licenses/
if command -v zip >/dev/null; then
  (cd dist && zip -qr cineo-licenses.zip cineo-licenses)
else
  (cd dist && python3 -m zipfile -c cineo-licenses.zip cineo-licenses)
fi
rm -rf dist/cineo-licenses

cp target/release/cineo-desktop dist/cineo-x86_64-linux
cp crates/cineo-desktop/assets/brand/icon.png dist/cineo.png
cp scripts/install.sh dist/
ls -l dist
