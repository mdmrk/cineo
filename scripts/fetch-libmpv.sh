#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/libmpv-windows.env"

out=$1
mkdir -p "$out"
archive="$out/$LIBMPV_FILE"
for url in "$LIBMPV_COPIES/latest/download" "$LIBMPV_COPIES/download/prerelease" "$LIBMPV_UPSTREAM"; do
  curl -fsSL --retry 3 -o "$archive" "$url/$LIBMPV_FILE" && break
done
echo "$LIBMPV_SHA256  $archive" | sha256sum -c -
7z e -y -o"$out" "$archive" libmpv-2.dll >/dev/null
gzip -9 -n -k -f "$out/libmpv-2.dll"
ls -l "$out/libmpv-2.dll" "$out/libmpv-2.dll.gz"
