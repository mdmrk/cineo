#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/libmpv-windows.env

tag=libmpv-windows-$LIBMPV_TAG
if gh release view "$tag" >/dev/null 2>&1; then
  echo "$tag already holds $LIBMPV_FILE"
  exit 0
fi
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
curl -fsSL --retry 5 --retry-all-errors -o "$dir/$LIBMPV_FILE" "$LIBMPV_UPSTREAM/$LIBMPV_FILE"
echo "$LIBMPV_SHA256  $dir/$LIBMPV_FILE" | sha256sum -c -
gh release create "$tag" "$dir/$LIBMPV_FILE" --prerelease --latest=false --target "${GITHUB_SHA:?}" \
  --title "libmpv for Windows ($LIBMPV_TAG)" \
  --notes "An unmodified copy of $LIBMPV_UPSTREAM/$LIBMPV_FILE (SHA-256 $LIBMPV_SHA256), kept because upstream deletes old releases. LGPL-2.1-or-later; mpv commit $LIBMPV_MPV_COMMIT; build log $LIBMPV_BUILD_LOG. Cineo's Windows build embeds the libmpv-2.dll it contains."
