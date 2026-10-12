#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dist

licenses() {
  local dir=$1
  mkdir -p "$dir"
  cp LICENSE crates/cineo-desktop/assets/fonts/*-OFL.txt \
    crates/cineo-desktop/assets/licenses/LGPL-2.1.txt "$dir/"
  local iconflow
  iconflow=$(cargo metadata --format-version 1 --locked \
    | jq -r '.packages[] | select(.name == "iconflow") | .manifest_path')
  if command -v cygpath >/dev/null; then iconflow=$(cygpath -u "$iconflow"); fi
  cp "$(dirname "$iconflow")/THIRD_PARTY_LICENSES_FONTS.md" "$dir/"
  source scripts/libmpv-windows.env
  cat > "$dir/libmpv.md" <<EOF
# libmpv (Windows builds only)

cineo-x86_64-windows.exe contains an unmodified LGPL build of libmpv-2.dll
(mpv and FFmpeg, LGPL-2.1-or-later; license text in LGPL-2.1.txt).

- Binary: ${LIBMPV_UPSTREAM}/${LIBMPV_FILE}
  (SHA-256 ${LIBMPV_SHA256})
- mpv source: https://github.com/mpv-player/mpv/commit/${LIBMPV_MPV_COMMIT}
- Build scripts and the sources of every library: https://github.com/zhongfly/mpv-winbuild
  (build log: ${LIBMPV_BUILD_LOG})

Cineo writes the DLL to %LOCALAPPDATA%\\Cineo\\cache\\libmpv\\<version>\\.
To use your own build, put a libmpv-2.dll next to cineo-x86_64-windows.exe;
it is loaded first.
EOF
}

case "$1" in
  extras)
    rm -rf dist/cineo-licenses
    licenses dist/cineo-licenses
    if command -v zip >/dev/null; then
      (cd dist && zip -qr cineo-licenses.zip cineo-licenses)
    else
      (cd dist && python3 -m zipfile -c cineo-licenses.zip cineo-licenses)
    fi
    rm -rf dist/cineo-licenses
    cp crates/cineo-desktop/assets/brand/icon.png dist/cineo.png
    cp scripts/install.sh scripts/install.ps1 dist/
    ;;
  x86_64-unknown-linux-gnu)
    cp "target/$1/release/cineo-desktop" dist/cineo-x86_64-linux
    ;;
  x86_64-pc-windows-msvc)
    cp "target/$1/release/cineo-desktop.exe" dist/cineo-x86_64-windows.exe
    ;;
  aarch64-apple-darwin)
    name=cineo-aarch64-macos
    rm -rf "dist/$name"
    licenses "dist/$name"
    cp "target/$1/release/cineo-desktop" README.md "dist/$name/"
    (cd dist && tar czf "$name.tar.gz" "$name" && rm -rf "$name")
    ;;
  *)
    echo "unknown target: $1" >&2
    exit 1
    ;;
esac
ls -l dist
