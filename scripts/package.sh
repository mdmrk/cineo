#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
rm -rf dist
mkdir -p dist

case "${1:-linux}" in
  linux)
    mkdir -p dist/cineo-licenses
    cp LICENSE crates/cineo-desktop/assets/fonts/*-OFL.txt \
      crates/cineo-desktop/assets/licenses/LGPL-2.1.txt dist/cineo-licenses/
    iconflow=$(cargo metadata --format-version 1 --locked \
      | jq -r '.packages[] | select(.name == "iconflow") | .manifest_path')
    cp "$(dirname "$iconflow")/THIRD_PARTY_LICENSES_FONTS.md" dist/cineo-licenses/
    source scripts/libmpv-windows.env
    cat > dist/cineo-licenses/libmpv.md <<EOT
# libmpv (Windows build only)

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
EOT
    if command -v zip >/dev/null; then
      (cd dist && zip -qr cineo-licenses.zip cineo-licenses)
    else
      (cd dist && python3 -m zipfile -c cineo-licenses.zip cineo-licenses)
    fi
    rm -rf dist/cineo-licenses
    cp target/release/cineo-desktop dist/cineo-x86_64-linux
    cp crates/cineo-desktop/assets/brand/icon.png dist/cineo.png
    cp scripts/install.sh scripts/install.ps1 dist/
    ;;
  windows)
    cp target/release/cineo-desktop.exe dist/cineo-x86_64-windows.exe
    ;;
  *)
    echo "unknown platform: $1" >&2
    exit 1
    ;;
esac
ls -l dist
