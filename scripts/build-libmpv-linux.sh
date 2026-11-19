#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
source "$root/scripts/libmpv-android.env"
source "$root/scripts/libmpv-linux.env"

stage=${CINEO_LIBMPV_STAGE:-/tmp/cineo-libmpv-linux}
src=$stage/src
build=$stage/build
prefix=$stage/prefix
out=$root/target/libmpv-linux/out
rm -rf "$build" "$prefix" "$out"
mkdir -p "$src" "$build" "$prefix" "$out/lib" "$out/licenses"

export SOURCE_DATE_EPOCH=0
export PKG_CONFIG_PATH=$prefix/lib/pkgconfig
export CFLAGS="-O2 -fPIC -ffile-prefix-map=$stage=."
export CXXFLAGS=$CFLAGS
jobs=$(nproc)

step() { printf '\n==> %s\n' "$*"; }

step "meson $MESON_VERSION"
if [[ ! -x $stage/venv/bin/meson ]] || [[ $("$stage/venv/bin/meson" --version) != "$MESON_VERSION" ]]; then
  python3 -m venv "$stage/venv"
  printf 'meson==%s --hash=sha256:%s\n' "$MESON_VERSION" "$MESON_SHA256" >"$stage/meson.txt"
  "$stage/venv/bin/pip" install --quiet --only-binary :all: --require-hashes -r "$stage/meson.txt"
fi
meson=$stage/venv/bin/meson

step "sources"
while read -r name url sha; do
  [[ -z $name ]] && continue
  archive=$src/$(basename "$url")
  if [[ ! -f $archive ]] || ! echo "$sha  $archive" | sha256sum -c --status -; then
    curl -fsSL --retry 5 --retry-all-errors -o "$archive" "$url"
    echo "$sha  $archive" | sha256sum -c -
  fi
  rm -rf "${src:?}/$name"
  mkdir -p "$src/$name"
  tar -xf "$archive" -C "$src/$name" --strip-components=1
  printf '%s %s %s\n' "$name" "$url" "$sha" >>"$out/SOURCES"
done <<<"$SOURCES
$LIBVA_SOURCE"

if [[ ! -e $src/libplacebo ]]; then
  git clone --quiet "$LIBPLACEBO_GIT" "$src/libplacebo"
fi
git -C "$src/libplacebo" fetch --quiet origin "$LIBPLACEBO_COMMIT"
git -C "$src/libplacebo" checkout --quiet --force "$LIBPLACEBO_COMMIT"
git -C "$src/libplacebo" submodule update --quiet --init --depth 1 \
  3rdparty/glad 3rdparty/jinja 3rdparty/markupsafe 3rdparty/fast_float
printf 'libplacebo %s %s\n' "$LIBPLACEBO_GIT" "$LIBPLACEBO_COMMIT" >>"$out/SOURCES"

publish_private() {
  python3 -I - "$prefix/lib/pkgconfig" <<'PY'
import pathlib, sys
for pc in pathlib.Path(sys.argv[1]).glob("*.pc"):
    fields, order, other = {}, [], []
    for line in pc.read_text().splitlines():
        key, sep, value = line.partition(":")
        if sep and key in ("Libs", "Libs.private", "Requires", "Requires.private"):
            fields[key] = value.strip()
            order.append(key)
        else:
            other.append(line)
    libs = " ".join(v for v in (fields.get("Libs"), fields.get("Libs.private")) if v)
    requires = ", ".join(v for v in (fields.get("Requires"), fields.get("Requires.private")) if v)
    body = other + ([f"Libs: {libs}"] if libs else []) + ([f"Requires: {requires}"] if requires else [])
    pc.write_text("\n".join(body) + "\n")
PY
}

meson_build() {
  local name=$1 library=$2
  shift 2
  step "$name"
  "$meson" setup "$build/$name" "$src/$name" --prefix "$prefix" --libdir lib \
    --buildtype release --default-library "$library" "$@"
  ninja -C "$build/$name" install >/dev/null
  publish_private
}

step "openssl"
mkdir -p "$build/openssl"
(
  cd "$build/openssl"
  "$src/openssl/Configure" linux-x86_64 --prefix="$prefix" --libdir=lib \
    no-shared no-tests no-apps no-docs no-module >/dev/null
  make -j"$jobs" build_libs >/dev/null
  make install_dev >/dev/null
)
publish_private
openssl_libs=$(pkg-config --libs openssl | xargs)
if [[ $openssl_libs != "-L$prefix/lib -lssl -lcrypto -ldl -pthread" ]]; then
  echo "pkg-config gives OpenSSL's libraries as '$openssl_libs'" >&2
  exit 1
fi

meson_build libva shared -Ddriverdir="$LIBVA_DRIVER_DIRS" -Ddisable_drm=false \
  -Dwith_x11=no -Dwith_glx=no -Dwith_wayland=no -Dwith_win32=no -Denable_docs=false
meson_build dav1d static -Denable_tools=false -Denable_tests=false -Denable_examples=false -Denable_docs=false
meson_build freetype static -Dbrotli=disabled -Dbzip2=disabled -Dharfbuzz=disabled -Dpng=disabled -Dzlib=disabled -Dtests=disabled
meson_build fribidi static -Ddocs=false -Dbin=false -Dtests=false
meson_build harfbuzz static -Dfreetype=enabled -Dglib=disabled -Dgobject=disabled -Dcairo=disabled \
  -Dchafa=disabled -Dicu=disabled -Dgraphite2=disabled -Dpng=disabled -Dzlib=disabled \
  -Dtests=disabled -Ddocs=disabled -Dutilities=disabled -Dintrospection=disabled -Dbenchmark=disabled
meson_build libass static -Dfontconfig=enabled -Drequire-system-font-provider=true -Dlibunibreak=disabled \
  -Dtest=disabled -Dcompare=disabled -Dprofile=disabled -Dfuzz=disabled -Dcheckasm=disabled

step "ffmpeg"
mkdir -p "$build/ffmpeg"
(
  cd "$build/ffmpeg"
  "$src/ffmpeg/configure" --prefix="$prefix" \
    --enable-shared --disable-static --enable-pic \
    --disable-programs --disable-doc --disable-avdevice --disable-vulkan \
    --disable-encoders --disable-muxers --disable-devices --disable-autodetect \
    --enable-zlib --enable-iconv --enable-openssl --enable-libdav1d --enable-vaapi --enable-libdrm \
    --extra-ldflags=-Wl,--exclude-libs,ALL >/dev/null
  make -j"$jobs" >/dev/null
  make install >/dev/null
)
publish_private

meson_build libplacebo static -Dvulkan=disabled -Dopengl=enabled -Dglslang=disabled -Dshaderc=disabled \
  -Dlcms=disabled -Dd3d11=disabled -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false

step "mpv"
"$meson" setup "$build/mpv" "$src/mpv" --prefix "$prefix" --libdir lib --buildtype release \
  --default-library shared -Dc_link_args="['-Wl,--no-undefined', '-Wl,--exclude-libs,ALL']" \
  -Dgpl=false -Dlibmpv=true -Dcplayer=false -Dtests=false -Dbuild-date=false \
  -Dlua=disabled -Djavascript=disabled -Diconv=enabled -Dzlib=enabled -Dlibarchive=disabled \
  -Dlibbluray=disabled -Ddvdnav=disabled -Ddvda=disabled -Dcdda=disabled -Duchardet=disabled \
  -Dlibcurl=disabled -Dsubrandr=disabled \
  -Drubberband=disabled -Dlcms2=disabled -Dzimg=disabled -Dvapoursynth=disabled \
  -Djpeg=disabled -Dlibavdevice=disabled -Dcplugins=disabled \
  -Dvulkan=disabled -Dshaderc=disabled -Dspirv-cross=disabled \
  -Dx11=disabled -Dwayland=disabled -Dgbm=disabled -Dvdpau=disabled \
  -Dgl=enabled -Degl=disabled -Ddrm=enabled -Dvaapi=enabled -Dvaapi-drm=enabled \
  -Dalsa=enabled -Dpulse=enabled -Dpipewire=disabled -Djack=disabled -Dopenal=disabled \
  -Dsdl2-audio=disabled -Dsdl2-video=disabled -Dsdl2-gamepad=disabled \
  -Dmanpage-build=disabled -Dhtml-build=disabled -Dpdf-build=disabled
ninja -C "$build/mpv" install >/dev/null

step "check"
if grep -q "^#define HAVE_GPL 1" "$build/mpv/config.h"; then
  echo "mpv was configured with GPL code" >&2
  exit 1
fi
if grep -qE -- '--enable-(gpl|version3|nonfree)' "$build/ffmpeg/ffbuild/config.mak"; then
  echo "FFmpeg was configured with a GPL, version3 or nonfree flag" >&2
  exit 1
fi
for feature in VAAPI VAAPI_DRM GL PULSE ALSA; do
  grep -q "^#define HAVE_$feature 1" "$build/mpv/config.h" || {
    echo "mpv was built without $feature" >&2
    exit 1
  }
done

host=" $LINUX_HOST_LIBS "
queue=(libmpv.so.2)
shipped=" "
while ((${#queue[@]})); do
  lib=${queue[0]}
  queue=("${queue[@]:1}")
  [[ $shipped == *" $lib "* ]] && continue
  [[ -e $prefix/lib/$lib ]] || {
    echo "$lib is needed but was not built" >&2
    exit 1
  }
  shipped+="$lib "
  cp -L "$prefix/lib/$lib" "$out/lib/$lib"
  while read -r needed; do
    if [[ -e $prefix/lib/$needed ]]; then
      queue+=("$needed")
    elif [[ $host != *" $needed "* ]]; then
      echo "$lib needs $needed, which is neither shipped nor a host library (LINUX_HOST_LIBS)" >&2
      exit 1
    fi
  done < <(readelf -d "$out/lib/$lib" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
done
[[ $shipped == *" libva-drm.so.2 "* ]] || {
  echo "libmpv does not use libva-drm.so.2, so VA-API over DRM would not work" >&2
  exit 1
}
for lib in "$out"/lib/*; do
  patchelf --set-rpath '$ORIGIN' "$lib"
done

cp "$src/ffmpeg/COPYING.LGPLv2.1" "$out/licenses/ffmpeg-LGPL-2.1.txt"
cp "$src/mpv/LICENSE.LGPL" "$out/licenses/mpv-LGPL-2.1.txt"
cp "$src/openssl/LICENSE.txt" "$out/licenses/openssl-Apache-2.0.txt"
cp "$src/libva/COPYING" "$out/licenses/libva-MIT.txt"
cp "$src/dav1d/COPYING" "$out/licenses/dav1d-BSD-2-Clause.txt"
cp "$src/libass/COPYING" "$out/licenses/libass-ISC.txt"
cp "$src/freetype/LICENSE.TXT" "$out/licenses/freetype.txt"
cp "$src/freetype/docs/FTL.TXT" "$out/licenses/freetype-FTL.txt"
cp "$src/fribidi/COPYING" "$out/licenses/fribidi-LGPL-2.1.txt"
cp "$src/harfbuzz/COPYING" "$out/licenses/harfbuzz-MIT.txt"
cp "$src/libplacebo/LICENSE" "$out/licenses/libplacebo-LGPL-2.1.txt"
cp "$src/libplacebo/3rdparty/glad/LICENSE" "$out/licenses/libplacebo-glad.txt"
cp "$src/libplacebo/3rdparty/fast_float/LICENSE-MIT" "$out/licenses/libplacebo-fast_float-MIT.txt"

{
  cat <<EOF
# libmpv (Linux AppImage)

The Cineo AppImage contains libmpv, FFmpeg and libva, built from
unmodified sources without GPL, version3 or nonfree code, so libmpv and
FFmpeg are LGPL-2.1-or-later (mpv-LGPL-2.1.txt and ffmpeg-LGPL-2.1.txt)
and libva is MIT (libva-MIT.txt). Copyright the mpv developers, the
FFmpeg developers and Intel. The other license files belong to the
libraries linked into them.

## Files

The AppImage carries these, unmodified except for an \$ORIGIN run path,
in usr/lib/cineo/:

EOF
  for lib in "$out"/lib/*; do
    printf -- '- %s: SHA-256 %s\n' "$(basename "$lib")" "$(sha256sum "$lib" | cut -d' ' -f1)"
  done
  cat <<EOF

## Source

They were built, unmodified, from these pinned upstream sources (name,
URL, SHA-256; libplacebo is a git commit):

EOF
  sed 's/^/- /' "$out/SOURCES"
  cat <<EOF

The script that built them is scripts/build-libmpv-linux.sh with
scripts/libmpv-linux.env and scripts/libmpv-android.env, at the tag of this
release in https://github.com/mdmrk/cineo, run on Ubuntu 24.04.

## Using your own build

Extract the AppImage (--appimage-extract), replace the libraries in
squashfs-root/usr/lib/cineo/ and run squashfs-root/AppRun, or put your own
libmpv.so.2 beside a copy of cineo-x86_64-linux: Cineo loads the libmpv
beside its executable first.
EOF
} >"$out/libmpv.md"

step "done"
ls -l "$out/lib"
