#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/release" "$work/bin"
minisign -G -W -p "$work/test.pub" -s "$work/test.key" >/dev/null
key=$(sed -n 2p "$work/test.pub")
sed "s|^minisign_key=.*|minisign_key=$key|" scripts/install.sh >"$work/install.sh"
cat >"$work/bin/curl" <<'CURL'
#!/bin/sh
while [ $# -gt 1 ]; do
  [ "$1" = -o ] && out=$2
  shift
done
cp "$RELEASE/$(basename "$1")" "$out" 2>/dev/null
CURL
chmod +x "$work/bin/curl"

release() {
  rm -rf "$work/release" "$work/home"
  mkdir -p "$work/release" "$work/home"
  printf '#!/bin/sh\n' >"$work/release/cineo-x86_64-linux"
  printf 'png' >"$work/release/cineo.png"
  (cd "$work/release" && python3 -I -m zipfile -c cineo-licenses.zip cineo.png &&
    sha256sum -- * >SHA256SUMS)
}

sign() {
  minisign -S -s "$work/test.key" -m "$work/release/SHA256SUMS" -x "$work/release/SHA256SUMS.minisig" >/dev/null
}

install() {
  env HOME="$work/home" XDG_BIN_HOME= XDG_DATA_HOME= XDG_STATE_HOME= RELEASE="$work/release" \
    PATH="$work/bin:$PATH" "$@" sh "$work/install.sh" >/dev/null 2>&1
}

failures=0
expect() {
  local outcome=$1 what=$2
  shift 2
  if install "$@"; then
    [[ $outcome == pass && -x $work/home/.local/bin/cineo-desktop ]] || {
      echo "FAIL: $what was installed" >&2
      failures=$((failures + 1))
    }
  else
    [[ $outcome == fail && ! -e $work/home/.local/bin/cineo-desktop ]] || {
      echo "FAIL: $what was not installed" >&2
      failures=$((failures + 1))
    }
  fi
}

release
sign
expect pass "a signed release"

release
sign
printf '0000000000000000000000000000000000000000000000000000000000000000  extra\n' >>"$work/release/SHA256SUMS"
expect fail "checksums changed after signing"

release
expect fail "an unsigned latest release"

release
expect pass "an unsigned release picked by version" CINEO_VERSION=v1.0.0

release
minisign -G -W -p "$work/other.pub" -s "$work/other.key" >/dev/null
minisign -S -s "$work/other.key" -m "$work/release/SHA256SUMS" -x "$work/release/SHA256SUMS.minisig" >/dev/null
expect fail "a release signed by another key"

release
sign
printf 'tampered' >"$work/release/cineo.png"
expect fail "a file that does not match its checksum"

((failures == 0)) || exit 1
echo "install.sh: all cases passed"
