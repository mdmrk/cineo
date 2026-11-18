#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

dir=${1:?usage: sign-release.sh DIR TAG}
tag=${2:?usage: sign-release.sh DIR TAG}
sums=$dir/SHA256SUMS
[[ -f $sums ]] || {
  echo "$sums is missing" >&2
  exit 1
}
[[ -n ${CINEO_MINISIGN_KEY:-} ]] || {
  echo "CINEO_MINISIGN_KEY is not set; refusing to publish unsigned checksums" >&2
  exit 1
}
[[ $(sed -n 1p <<<"$CINEO_MINISIGN_KEY") == "untrusted comment:"* && -n $(sed -n 2p <<<"$CINEO_MINISIGN_KEY") ]] || {
  echo "CINEO_MINISIGN_KEY must hold the whole secret key file: its 'untrusted comment:' line and the key line below it" >&2
  exit 1
}
key=$(mktemp)
trap 'rm -f "$key"' EXIT
chmod 600 "$key"
printf '%s\n' "$CINEO_MINISIGN_KEY" >"$key"
minisign -S -s "$key" -m "$sums" -x "$sums.minisig" -t "cineo $tag SHA256SUMS" >/dev/null
minisign -V -q -p minisign.pub -m "$sums" -x "$sums.minisig"
