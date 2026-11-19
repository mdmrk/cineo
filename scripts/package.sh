#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/common.sh
rm -rf dist
mkdir -p dist
linux_glibc_max=2.39
version=$(workspace_version)

check_version() {
  local reported
  reported=$("$1" --version)
  [[ $reported == "cineo-desktop $version" ]] || die "$1 reports '$reported', but Cargo.toml says $version"
}

check_glibc() {
  local needed
  needed=$(objdump -T "$1" | grep -oE 'GLIBC_[0-9]+(\.[0-9]+)+' | sed 's/^GLIBC_//' | sort -Vu | tail -n 1)
  [[ -n $needed ]] || die "$1 links no glibc symbol"
  [[ $(printf '%s\n' "$needed" "$linux_glibc_max" | sort -V | tail -n 1) == "$linux_glibc_max" ]] ||
    die "$1 needs glibc $needed, above the $linux_glibc_max that Linux releases support"
}

common_licenses() {
  cp LICENSE "$1/cineo-MIT.txt"
  cp crates/cineo-desktop/assets/fonts/*-OFL.txt "$(crate_dir iconflow)/THIRD_PARTY_LICENSES_FONTS.md" "$1/"
}

case "${1:-linux}" in
  linux)
    mkdir -p dist/cineo-licenses
    common_licenses dist/cineo-licenses
    mkdir dist/cineo-licenses/libmpv
    cp scripts/windows/licenses/* scripts/windows/licenses.txt dist/cineo-licenses/libmpv/
    source scripts/libmpv-windows.env
    cat >dist/cineo-licenses/libmpv.md <<EOT
# libmpv (Windows build only)

cineo-x86_64-windows.exe contains an unmodified LGPL build of libmpv-2.dll:
mpv is LGPL-2.1-or-later (libmpv/mpv-LGPL-2.1.txt, libmpv/mpv-Copyright.txt),
and FFmpeg is built with --enable-version3, so it is LGPL-3.0-or-later
(libmpv/ffmpeg-LGPL-3.0.txt, which builds on libmpv/ffmpeg-GPL-3.0.txt). The
other libraries it is built with, and their license texts, are in libmpv/;
libmpv/licenses.txt names the repository, commit and file of each text.

- Binary: ${LIBMPV_UPSTREAM}/${LIBMPV_FILE}
  (SHA-256 ${LIBMPV_SHA256}); every Cineo release carries the same file
- mpv source: https://github.com/mpv-player/mpv/commit/${LIBMPV_MPV_COMMIT}
- Build scripts and the sources of every library: https://github.com/zhongfly/mpv-winbuild
  (build log: ${LIBMPV_BUILD_LOG})

Cineo writes the DLL to %LOCALAPPDATA%\\Cineo\\cache\\libmpv\\<CRC32>\\.
To use your own build, put a libmpv-2.dll next to cineo-x86_64-windows.exe;
it is loaded first.
EOT
    (cd dist && python3 -I -m zipfile -c cineo-licenses.zip cineo-licenses)
    rm -rf dist/cineo-licenses
    cp target/release/cineo-desktop dist/cineo-x86_64-linux
    check_version dist/cineo-x86_64-linux
    check_glibc dist/cineo-x86_64-linux
    cp crates/cineo-desktop/assets/brand/icon.png dist/cineo.png
    cp scripts/install.sh scripts/install.ps1 dist/
    ;;
  windows)
    source scripts/libmpv-windows.env
    cp target/release/cineo-desktop.exe dist/cineo-x86_64-windows.exe
    check_version dist/cineo-x86_64-windows.exe
    cp "$(dirname "${CINEO_LIBMPV_DLL_GZ:?set CINEO_LIBMPV_DLL_GZ as for the build}")/$LIBMPV_FILE" dist/
    ;;
  android)
    apk=dist/cineo-arm64-android.apk
    libmpv=target/libmpv-android
    cp android/app/build/outputs/apk/release/app-release.apk "$apk"
    entries=$(unzip -Z1 "$apk")
    for required in assets/licenses/libmpv.md assets/licenses/mpv-LGPL-2.1.txt lib/arm64-v8a/libcineo_android.so; do
      grep -qxF "$required" <<<"$entries" || die "$apk has no $required"
    done
    named=$(sed -n 's/^- \(lib[^:]*\.so\): SHA-256 \([0-9a-f]\{64\}\)$/\1 \2/p' "$libmpv/out/libmpv.md")
    grep -q '^libmpv\.so ' <<<"$named" || die "$libmpv/out/libmpv.md names no libmpv.so"
    while read -r lib sha; do
      [[ $(unzip -p "$apk" "lib/arm64-v8a/$lib" | sha256sum | cut -d' ' -f1) == "$sha" ]] ||
        die "$apk: lib/arm64-v8a/$lib is missing or not the one libmpv.md names"
    done <<<"$named"
    libs=$(mktemp -d)
    unzip -q "$apk" 'lib/arm64-v8a/*.so' -d "$libs"
    for so in "$libs"/lib/arm64-v8a/*.so; do
      text=$(strings -n 8 "$so")
      name=$(basename "$so")
      for leak in "$PWD" "$HOME" "${CARGO_HOME:-$HOME/.cargo}" "${RUSTUP_HOME:-$HOME/.rustup}"; do
        ! grep -qF -- "$leak" <<<"$text" || die "$apk: $name holds the build path $leak, so the build is not reproducible"
      done
      ! grep 'built on: ' <<<"$text" | grep -qv 'built on: Thu Jan  1 00:00:00 1970 UTC' ||
        die "$apk: $name holds a build date, so the build is not reproducible"
      aligns=$(readelf -lW "$so" | awk '$1 == "LOAD" { print $NF }')
      [[ -n $aligns ]] || die "$apk: $name has no LOAD segment"
      for align in $aligns; do
        ((align >= 16384)) || die "$apk: $name has a LOAD segment aligned to $align, below 16 KB"
      done
    done
    rm -rf "$libs"
    sdk=${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}
    build_tools=$(find "$sdk/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)
    signer=$(sed -n 's/^AllowedAPKSigningKeys: //p' docs/fdroid/io.github.mdmrk.cineo.yml)
    digests=$("$build_tools/apksigner" verify --print-certs "$apk" | sed -n 's/.*certificate SHA-256 digest: //p' | sort -u)
    [[ $digests == "$signer" ]] || die "$apk is signed by ${digests:-nothing}, not the release key $signer that F-Droid allows"
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
    ;;
  macos)
    source scripts/libmpv-macos.env
    libmpv=target/libmpv-macos
    work=$(mktemp -d)
    mkdir "$work/stage"
    app=$work/stage/Cineo.app
    contents=$app/Contents
    mkdir -p "$contents/MacOS" "$contents/Frameworks" "$contents/Resources/licenses"
    lipo -create target/aarch64-apple-darwin/release/cineo-desktop \
      target/x86_64-apple-darwin/release/cineo-desktop -output "$contents/MacOS/cineo"
    check_version "$contents/MacOS/cineo"
    sed -e "s/__VERSION__/$version/g" -e "s/__BUILD__/${version%%-*}/g" \
      scripts/macos/Info.plist >"$contents/Info.plist"
    plutil -lint "$contents/Info.plist"
    iconset=$work/cineo.iconset
    mkdir -p "$iconset"
    icon=crates/cineo-desktop/assets/brand/icon.png
    for size in 16 32 128; do
      sips -z "$size" "$size" "$icon" --out "$iconset/icon_${size}x${size}.png" >/dev/null
      sips -z $((size * 2)) $((size * 2)) "$icon" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
    done
    cp "$icon" "$iconset/icon_256x256.png"
    iconutil -c icns "$iconset" -o "$contents/Resources/cineo.icns"
    cp "$libmpv"/lib/*.dylib "$contents/Frameworks/"
    for lib in "$contents"/Frameworks/*.dylib; do
      chmod u+w "$lib"
      install_name_tool -add_rpath @loader_path "$lib"
      codesign --force --sign - "$lib"
    done
    licenses=$contents/Resources/licenses
    common_licenses "$licenses"
    cp "$libmpv"/licenses/* "$licenses/"
    cat >"$licenses/libmpv.md" <<EOT
# libmpv (macOS build)

Cineo.app/Contents/Frameworks holds unmodified LGPL builds of libmpv
${LIBMPV_MACOS_MPV_VERSION} and FFmpeg ${LIBMPV_MACOS_FFMPEG_VERSION} with the libraries they use, from
media-kit/libmpv-darwin-build ${LIBMPV_MACOS_TAG} (variant video-default).
mpv is LGPL-2.1-or-later; FFmpeg is built with --enable-version3, so it is
LGPL-3.0-or-later. License texts are in this folder.

- Binary: ${LIBMPV_MACOS_UPSTREAM}/${LIBMPV_MACOS_FILE}
  (SHA-256 ${LIBMPV_MACOS_SHA256})
- Build scripts and the source of every library:
  https://github.com/media-kit/libmpv-darwin-build/tree/${LIBMPV_MACOS_TAG}
  (sources: packages.lock.nix)

The only change Cineo makes is an @loader_path run path in each library, so
they find each other inside the app. To use your own build, replace the
.dylib files in Cineo.app/Contents/Frameworks and sign the app again with
\`codesign --force --deep --sign - Cineo.app\`.
EOT
    codesign --force --sign - "$app"
    codesign --verify --strict --deep "$app"
    check=$work/dlopen
    printf '%s\n' '#include <dlfcn.h>' '#include <stdio.h>' \
      'int main(int argc, char **argv) { (void)argc; if (!dlopen(argv[1], RTLD_NOW)) { fprintf(stderr, "%s\n", dlerror()); return 1; } return 0; }' |
      cc -x c -o "$check" -
    "$check" "$contents/Frameworks/libmpv.dylib"
    for binary in "$contents/MacOS/cineo" "$contents"/Frameworks/*.dylib; do
      lipo "$binary" -verify_arch arm64 x86_64
    done
    ln -s /Applications "$work/stage/Applications"
    hdiutil create -volname Cineo -srcfolder "$work/stage" -format UDZO dist/cineo-universal-macos.dmg
    rm -rf "$work"
    ;;
  *) die "unknown platform: $1" ;;
esac
ls -l dist
