#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/libmpv-android.env

for name in CINEO_KEYSTORE CINEO_KEYSTORE_PASSWORD CINEO_KEY_ALIAS CINEO_KEY_PASSWORD; do
  if [[ -z ${!name:-} ]]; then
    echo "set $name: an unsigned APK is never released" >&2
    exit 1
  fi
done
if [[ -z ${ANDROID_HOME:-} && -f android/local.properties ]]; then
  ANDROID_HOME=$(sed -n 's/^sdk\.dir=//p' android/local.properties)
fi
export ANDROID_HOME=${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}
export CINEO_VERSION_CODE=${CINEO_VERSION_CODE:-$(git rev-list --count HEAD)}

libmpv=target/libmpv-android
pins=$(cat scripts/build-libmpv-android.sh scripts/libmpv-android.env | sha256sum | cut -d' ' -f1)
if [[ $(cat "$libmpv/pins" 2>/dev/null) != "$pins" || ! -f $libmpv/out/libmpv.md ||
  ! -f $libmpv/libmpv-android-sources.tar.xz ]]; then
  ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION" scripts/build-libmpv-android.sh
  echo "$pins" >"$libmpv/pins"
else
  echo "libmpv is up to date with its pins"
fi

echo "versionCode $CINEO_VERSION_CODE"
(cd android && ./gradlew assembleRelease)
scripts/package.sh android
