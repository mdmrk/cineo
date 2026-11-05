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
  android)
    apk=dist/cineo-arm64-android.apk
    libmpv=target/libmpv-android
    cp android/app/build/outputs/apk/release/app-release.apk "$apk"
    entries=$(unzip -Z1 "$apk")
    for required in assets/licenses/libmpv.md assets/licenses/mpv-LGPL-2.1.txt lib/arm64-v8a/libcineo_android.so; do
      grep -qxF "$required" <<<"$entries" || { echo "$apk has no $required" >&2; exit 1; }
    done
    for lib in libmpv.so libavcodec.so libavfilter.so libavformat.so libavutil.so libswresample.so libswscale.so; do
      shipped=$(unzip -p "$apk" "lib/arm64-v8a/$lib" | sha256sum | cut -d' ' -f1)
      grep -qF -- "- $lib: SHA-256 $shipped" "$libmpv/out/libmpv.md" \
        || { echo "$apk: lib/arm64-v8a/$lib is missing or not the one libmpv.md names" >&2; exit 1; }
    done
    libs=$(mktemp -d)
    unzip -q "$apk" 'lib/arm64-v8a/*.so' -d "$libs"
    for so in "$libs"/lib/arm64-v8a/*.so; do
      text=$(strings -n 8 "$so")
      for leak in "$PWD" "$HOME" "${CARGO_HOME:-$HOME/.cargo}" "${RUSTUP_HOME:-$HOME/.rustup}"; do
        if grep -qF -- "$leak" <<<"$text"; then
          echo "$apk: $(basename "$so") holds the build path $leak, so the build is not reproducible" >&2
          exit 1
        fi
      done
      if grep 'built on: ' <<<"$text" | grep -qv 'built on: Thu Jan  1 00:00:00 1970 UTC'; then
        echo "$apk: $(basename "$so") holds a build date, so the build is not reproducible" >&2
        exit 1
      fi
      aligns=$(readelf -lW "$so" | awk '$1 == "LOAD" { print $NF }')
      [[ -n $aligns ]] || { echo "$apk: $(basename "$so") has no LOAD segment" >&2; exit 1; }
      for align in $aligns; do
        ((align >= 16384)) || { echo "$apk: $(basename "$so") has a LOAD segment aligned to $align, below 16 KB" >&2; exit 1; }
      done
    done
    rm -rf "$libs"
    sdk=${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}
    build_tools=$(find "$sdk/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
    "$build_tools/apksigner" verify "$apk"
    python3 -I - "$apk" <<'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
eocd = data.rfind(b"PK\x05\x06")
cd = struct.unpack("<I", data[eocd + 16:eocd + 20])[0]
assert data[cd - 16:cd] == b"APK Sig Block 42", "the APK has no signing block"
at = cd - struct.unpack("<Q", data[cd - 24:cd - 16])[0]
while at < cd - 24:
    length, block = struct.unpack("<QI", data[at:at + 12])
    if block == 0x504B4453:
        sys.exit(f"{sys.argv[1]}: the signing block carries Google's encrypted dependency info")
    at += 8 + length
PY
    version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
    IFS=. read -r major minor patch <<<"$version"
    expected=$((major * 10000 + minor * 100 + patch))
    badging=$("$build_tools/aapt2" dump badging "$apk" | head -n 1)
    grep -qF "versionCode='$expected' versionName='$version'" <<<"$badging" \
      || { echo "$apk: expected versionCode $expected and versionName $version, got: $badging" >&2; exit 1; }
    ;;
  *)
    echo "unknown platform: $1" >&2
    exit 1
    ;;
esac
ls -l dist
