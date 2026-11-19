#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
usage="usage: fetch-libmpv.sh windows|macos DIR"
platform=${1:?$usage}
out=${2:?$usage}
case $platform in
  windows)
    source "$here/libmpv-windows.env"
    mkdir -p "$out"
    archive="$out/$LIBMPV_FILE"
    for url in "$LIBMPV_COPIES/latest/download" "$LIBMPV_COPIES/download/prerelease" "$LIBMPV_UPSTREAM"; do
      curl -fsSL --retry 3 -o "$archive" "$url/$LIBMPV_FILE" && break
    done
    echo "$LIBMPV_SHA256  $archive" | sha256sum -c -
    7z e -y -o"$out" "$archive" libmpv-2.dll >/dev/null
    gzip -9 -n -k -f "$out/libmpv-2.dll"
    ls -l "$out/libmpv-2.dll" "$out/libmpv-2.dll.gz"
    ;;
  macos)
    source "$here/libmpv-macos.env"
    mkdir -p "$out/lib" "$out/licenses"
    archive="$out/$LIBMPV_MACOS_FILE"
    curl -fsSL --retry 5 --retry-all-errors -o "$archive" "$LIBMPV_MACOS_UPSTREAM/$LIBMPV_MACOS_FILE"
    echo "$LIBMPV_MACOS_SHA256  $archive" | shasum -a 256 -c -
    tar -xzf "$archive" -C "$out/lib" --strip-components 1
    while read -r name url sha; do
      [[ -n "$name" ]] || continue
      curl -fsSL --retry 5 --retry-all-errors -o "$out/licenses/$name" "$url"
      echo "$sha  $out/licenses/$name" | shasum -a 256 -c -
    done <"$here/macos/licenses.txt"
    ls -l "$out/lib/libmpv.dylib"
    ;;
  *) echo "$usage" >&2 && exit 1 ;;
esac
