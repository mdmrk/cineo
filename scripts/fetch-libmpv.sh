#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/libmpv-windows.env"

out=$1
mkdir -p "$out"
archive="$out/$LIBMPV_FILE"
curl -fsSL --retry 3 -o "$archive" "$LIBMPV_UPSTREAM/$LIBMPV_FILE"
echo "$LIBMPV_SHA256  $archive" | sha256sum -c -
7z e -y -o"$out" "$archive" libmpv-2.dll >/dev/null
ls -l "$out/libmpv-2.dll"
