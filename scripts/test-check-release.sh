#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/scripts"
cp scripts/check-release.sh "$work/scripts/"
printf '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "2.3.4"\nedition = "2024"\n\n[workspace.metadata.android]\nversion = "9.9.9"\n' >"$work/Cargo.toml"

failures=0
expect() {
  local outcome=$1 what=$2
  shift 2
  if "$work/scripts/check-release.sh" "$@" >/dev/null 2>&1; then
    [[ $outcome == pass ]] || {
      echo "FAIL: $what was accepted" >&2
      failures=$((failures + 1))
    }
  else
    [[ $outcome == fail ]] || {
      echo "FAIL: $what was refused" >&2
      failures=$((failures + 1))
    }
  fi
}

expect pass "a tag naming the workspace version" version v2.3.4
expect fail "a tag naming another version" version v2.3.5
expect fail "a tag naming the Android metadata version" version v9.9.9
expect fail "a prerelease tag of a plain version" version v2.3.4-rc.1
printf '[workspace.package]\nversion = "2.3.4-rc.1"\n' >"$work/Cargo.toml"
expect pass "a prerelease tag naming the workspace version" version v2.3.4-rc.1
expect fail "a plain tag of a prerelease version" version v2.3.4
expect fail "no tag" version

((failures == 0)) || exit 1
echo "check-release.sh: all cases passed"
