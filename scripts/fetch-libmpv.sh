#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
source "$here/common.sh"
usage="usage: fetch-libmpv.sh windows|macos DIR"
platform=${1:?$usage}
out=${2:?$usage}
case $platform in
  windows)
    source "$here/libmpv-windows.env"
    mkdir -p "$out"
    fetch "$out/$LIBMPV_FILE" "$LIBMPV_SHA256" \
      "$LIBMPV_COPIES/latest/download/$LIBMPV_FILE" \
      "$LIBMPV_COPIES/download/prerelease/$LIBMPV_FILE" \
      "$LIBMPV_UPSTREAM/$LIBMPV_FILE"
    7z e -y -o"$out" "$out/$LIBMPV_FILE" libmpv-2.dll >/dev/null
    gzip -9 -n -k -f "$out/libmpv-2.dll"
    ls -l "$out/libmpv-2.dll" "$out/libmpv-2.dll.gz"
    ;;
  macos)
    source "$here/libmpv-macos.env"
    mkdir -p "$out/lib" "$out/licenses"
    fetch "$out/$LIBMPV_MACOS_FILE" "$LIBMPV_MACOS_SHA256" "$LIBMPV_MACOS_UPSTREAM/$LIBMPV_MACOS_FILE"
    tar -xzf "$out/$LIBMPV_MACOS_FILE" -C "$out/lib" --strip-components 1
    while read -r name url sha; do
      [[ -n "$name" ]] || continue
      fetch "$out/licenses/$name" "$sha" "$url"
    done <"$here/macos/licenses.txt"
    ls -l "$out/lib/libmpv.dylib"
    ;;
  *) die "$usage" ;;
esac
