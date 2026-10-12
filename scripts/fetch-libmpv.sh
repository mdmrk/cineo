#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/libmpv-windows.env"

out=$1
mkdir -p "$out"
archive="$out/$LIBMPV_FILE"
mirror="https://github.com/${GITHUB_REPOSITORY:-mdmrk/cineo}/releases/download/libmpv-${LIBMPV_TAG}/${LIBMPV_FILE}"

if ! curl -fsSL --retry 3 -o "$archive" "$mirror"; then
  echo "no mirror; downloading from upstream"
  curl -fsSL --retry 3 -o "$archive" "$LIBMPV_UPSTREAM/$LIBMPV_FILE"
fi
echo "$LIBMPV_SHA256  $archive" | sha256sum -c -
7z e -y -o"$out" "$archive" libmpv-2.dll >/dev/null
ls -l "$out/libmpv-2.dll"
