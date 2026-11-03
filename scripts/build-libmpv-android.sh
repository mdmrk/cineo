#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
source "$root/scripts/libmpv-android.env"

ndk=${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to NDK $NDK_VERSION}
if ! grep -qx "Pkg.Revision = $NDK_VERSION" "$ndk/source.properties"; then
  echo "expected NDK $NDK_VERSION at $ndk" >&2
  exit 1
fi

work=$(realpath -m "${1:-$root/target/libmpv-android}")
src=$work/src
build=$work/build
prefix=$work/prefix
out=$work/out
rm -rf "$build" "$prefix" "$out"
mkdir -p "$src" "$build" "$prefix" "$out/lib" "$out/licenses"

toolchain=$ndk/toolchains/llvm/prebuilt/linux-x86_64
triple=aarch64-linux-android
cc=$toolchain/bin/$triple$ANDROID_API-clang
cxx=$toolchain/bin/$triple$ANDROID_API-clang++
export PATH="$toolchain/bin:$PATH"
export PKG_CONFIG_LIBDIR=$prefix/lib/pkgconfig
unset PKG_CONFIG_PATH PKG_CONFIG_SYSROOT_DIR
jobs=$(nproc)

step() { printf '\n==> %s\n' "$*"; }

step "meson $MESON_VERSION"
if [[ ! -x $work/venv/bin/meson ]] || [[ $("$work/venv/bin/meson" --version) != "$MESON_VERSION" ]]; then
  python3 -m venv "$work/venv"
  "$work/venv/bin/pip" install --quiet "meson==$MESON_VERSION"
fi
meson=$work/venv/bin/meson

step "sources"
while read -r name url sha; do
  [[ -z $name ]] && continue
  archive=$src/$(basename "$url")
  if [[ ! -f $archive ]] || ! echo "$sha  $archive" | sha256sum -c --status -; then
    curl -fsSL --retry 3 -o "$archive" "$url"
    echo "$sha  $archive" | sha256sum -c -
  fi
  rm -rf "${src:?}/$name"
  mkdir -p "$src/$name"
  tar -xf "$archive" -C "$src/$name" --strip-components=1
  printf '%s %s %s\n' "$name" "$url" "$sha" >>"$out/SOURCES"
done <<<"$SOURCES"

if [[ ! -d $src/libplacebo/.git ]]; then
  git clone --quiet "$LIBPLACEBO_GIT" "$src/libplacebo"
fi
git -C "$src/libplacebo" fetch --quiet origin "$LIBPLACEBO_COMMIT"
git -C "$src/libplacebo" checkout --quiet --force "$LIBPLACEBO_COMMIT"
git -C "$src/libplacebo" submodule update --quiet --init --depth 1 \
  3rdparty/glad 3rdparty/jinja 3rdparty/markupsafe 3rdparty/fast_float
printf 'libplacebo %s %s\n' "$LIBPLACEBO_GIT" "$LIBPLACEBO_COMMIT" >>"$out/SOURCES"

cat >"$work/cross.ini" <<EOF
[binaries]
c = '$cc'
cpp = '$cxx'
ar = '$toolchain/bin/llvm-ar'
strip = '$toolchain/bin/llvm-strip'
pkg-config = 'pkg-config'

[host_machine]
system = 'android'
cpu_family = 'aarch64'
cpu = 'armv8-a'
endian = 'little'
EOF

meson_build() {
  local name=$1
  shift
  step "$name"
  "$meson" setup "$build/$name" "$src/$name" --cross-file "$work/cross.ini" \
    --prefix "$prefix" --libdir lib --buildtype release --default-library static "$@"
  ninja -C "$build/$name" install >/dev/null
}

step "openssl"
mkdir -p "$build/openssl"
(
  cd "$build/openssl"
  ANDROID_NDK_ROOT=$ndk "$src/openssl/Configure" android-arm64 \
    "-D__ANDROID_API__=$ANDROID_API" --prefix="$prefix" --libdir=lib \
    no-shared no-tests no-apps no-docs no-module >/dev/null
  make -j"$jobs" build_libs >/dev/null
  make install_dev >/dev/null
)

meson_build dav1d -Denable_tools=false -Denable_tests=false -Denable_examples=false -Denable_docs=false
meson_build freetype -Dbrotli=disabled -Dbzip2=disabled -Dharfbuzz=disabled -Dpng=disabled -Dzlib=disabled -Dtests=disabled
meson_build fribidi -Ddocs=false -Dbin=false -Dtests=false
meson_build harfbuzz -Dfreetype=enabled -Dglib=disabled -Dgobject=disabled -Dcairo=disabled \
  -Dchafa=disabled -Dicu=disabled -Dgraphite2=disabled -Dpng=disabled -Dzlib=disabled \
  -Dtests=disabled -Ddocs=disabled -Dutilities=disabled -Dintrospection=disabled -Dbenchmark=disabled
meson_build libass -Dfontconfig=disabled -Drequire-system-font-provider=false -Dlibunibreak=disabled \
  -Dtest=disabled -Dcompare=disabled -Dprofile=disabled -Dfuzz=disabled -Dcheckasm=disabled

step "ffmpeg"
mkdir -p "$build/ffmpeg"
(
  cd "$build/ffmpeg"
  "$src/ffmpeg/configure" --prefix="$prefix" \
    --target-os=android --arch=aarch64 --cpu=armv8-a --enable-cross-compile \
    --cc="$cc" --cxx="$cxx" --nm=llvm-nm --ar=llvm-ar --ranlib=llvm-ranlib --strip=llvm-strip \
    --pkg-config=pkg-config --pkg-config-flags=--static \
    --enable-shared --disable-static --enable-pic \
    --disable-programs --disable-doc --disable-avdevice --disable-vulkan \
    --disable-encoders --disable-muxers --disable-devices \
    --enable-jni --enable-mediacodec --enable-openssl --enable-libdav1d >/dev/null
  make -j"$jobs" >/dev/null
  make install >/dev/null
)

meson_build libplacebo -Dvulkan=disabled -Dopengl=enabled -Dglslang=disabled -Dshaderc=disabled \
  -Dlcms=disabled -Dd3d11=disabled -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false

step "mpv"
"$meson" setup "$build/mpv" "$src/mpv" --cross-file "$work/cross.ini" \
  --prefix "$prefix" --libdir lib --buildtype release --default-library shared -Dprefer_static=true \
  -Dc_link_args="['-Wl,--no-undefined', '-lc++_static', '-lc++abi']" \
  -Dgpl=false -Dlibmpv=true -Dcplayer=false -Dtests=false -Dbuild-date=false \
  -Dlua=disabled -Djavascript=disabled -Diconv=disabled -Dlibarchive=disabled \
  -Dlibbluray=disabled -Ddvdnav=disabled -Dcdda=disabled -Duchardet=disabled \
  -Drubberband=disabled -Dlcms2=disabled -Dzimg=disabled -Dvapoursynth=disabled \
  -Djpeg=disabled -Dlibavdevice=disabled -Dcplugins=disabled \
  -Dvulkan=disabled -Dshaderc=disabled -Dspirv-cross=disabled \
  -Degl-android=enabled -Dandroid-media-ndk=enabled \
  -Daudiotrack=enabled -Daaudio=enabled -Dopensles=enabled \
  -Dmanpage-build=disabled -Dhtml-build=disabled -Dpdf-build=disabled
ninja -C "$build/mpv" install >/dev/null

step "check"
libs=(libmpv.so libavcodec.so libavfilter.so libavformat.so libavutil.so libswresample.so libswscale.so)
for lib in "${libs[@]}"; do
  cp -L "$prefix/lib/$lib" "$out/lib/$lib"
done

if strings "$out/lib/libavcodec.so" | grep -qE -- '--enable-(gpl|version3|nonfree)'; then
  echo "FFmpeg was configured with a GPL, version3 or nonfree flag" >&2
  exit 1
fi
if grep -q "^#define HAVE_GPL 1" "$build/mpv/config.h"; then
  echo "mpv was configured with GPL code" >&2
  exit 1
fi
shipped=" ${libs[*]} "
system=" libc.so libm.so libdl.so liblog.so libandroid.so libEGL.so libGLESv2.so libGLESv3.so libOpenSLES.so libaaudio.so libmediandk.so libz.so "
for lib in "$out"/lib/*.so; do
  while read -r needed; do
    if [[ $shipped != *" $needed "* && $system != *" $needed "* ]]; then
      echo "$(basename "$lib") needs $needed, which is neither shipped nor part of Android" >&2
      exit 1
    fi
  done < <(llvm-readelf -d "$lib" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
done

cp "$src/ffmpeg/COPYING.LGPLv2.1" "$out/licenses/ffmpeg-LGPL-2.1.txt"
cp "$src/mpv/LICENSE.LGPL" "$out/licenses/mpv-LGPL-2.1.txt"
cp "$src/openssl/LICENSE.txt" "$out/licenses/openssl-Apache-2.0.txt"
cp "$src/dav1d/COPYING" "$out/licenses/dav1d-BSD-2-Clause.txt"
cp "$src/libass/COPYING" "$out/licenses/libass-ISC.txt"
cp "$src/freetype/LICENSE.TXT" "$out/licenses/freetype.txt"
cp "$src/freetype/docs/FTL.TXT" "$out/licenses/freetype-FTL.txt"
cp "$src/fribidi/COPYING" "$out/licenses/fribidi-LGPL-2.1.txt"
cp "$src/harfbuzz/COPYING" "$out/licenses/harfbuzz-MIT.txt"
cp "$src/libplacebo/LICENSE" "$out/licenses/libplacebo-LGPL-2.1.txt"

archive=$work/libmpv-android-arm64-v8a.tar.xz
tar -C "$out" -cJf "$archive" lib licenses SOURCES
step "done"
sha256sum "$archive"
ls -l "$out/lib"
