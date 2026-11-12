#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source "$here/libmpv-macos.env"

out=$1
mkdir -p "$out/lib" "$out/licenses"
archive="$out/$LIBMPV_MACOS_FILE"
curl -fsSL --retry 5 --retry-all-errors -o "$archive" "$LIBMPV_MACOS_UPSTREAM/$LIBMPV_MACOS_FILE"
echo "$LIBMPV_MACOS_SHA256  $archive" | shasum -a 256 -c -
tar -xzf "$archive" -C "$out/lib" --strip-components 1
while read -r name url; do
  [[ -n "$name" ]] || continue
  curl -fsSL --retry 5 --retry-all-errors -o "$out/licenses/$name" "$url"
done <"$here/macos/licenses.txt"
ls -l "$out/lib/libmpv.dylib"
