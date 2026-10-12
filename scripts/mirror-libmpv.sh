#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/libmpv-windows.env"

tag="libmpv-${LIBMPV_TAG}"
if gh release view "$tag" --repo "$GITHUB_REPOSITORY" >/dev/null 2>&1; then
  exit 0
fi
gh release create "$tag" "$1/$LIBMPV_FILE" --repo "$GITHUB_REPOSITORY" \
  --target "$GITHUB_SHA" --prerelease --latest=false --title "libmpv ${LIBMPV_TAG} (mirror)" \
  --notes "Unmodified copy of ${LIBMPV_UPSTREAM}/${LIBMPV_FILE} (SHA-256 ${LIBMPV_SHA256}), bundled in Cineo for Windows. mpv commit ${LIBMPV_MPV_COMMIT}; build log ${LIBMPV_BUILD_LOG}. LGPL-2.1-or-later."
