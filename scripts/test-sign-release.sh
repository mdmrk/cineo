#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/scripts" "$work/dist"
cp scripts/sign-release.sh "$work/scripts/"
minisign -G -W -p "$work/minisign.pub" -s "$work/secret.key" >/dev/null
printf 'abc  cineo-x86_64-linux\n' >"$work/dist/SHA256SUMS"

failures=0
fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

if CINEO_MINISIGN_KEY='' "$work/scripts/sign-release.sh" "$work/dist" v1.0.0 >/dev/null 2>&1; then
  fail "checksums were signed without a key"
fi
[[ ! -e $work/dist/SHA256SUMS.minisig ]] || fail "a signature was left without a key"

CINEO_MINISIGN_KEY=$(cat "$work/secret.key") "$work/scripts/sign-release.sh" "$work/dist" v1.0.0 ||
  fail "signing with the key failed"
minisign -V -q -p "$work/minisign.pub" -m "$work/dist/SHA256SUMS" -x "$work/dist/SHA256SUMS.minisig" ||
  fail "the signature does not verify"
grep -qF "trusted comment: cineo v1.0.0 SHA256SUMS" "$work/dist/SHA256SUMS.minisig" ||
  fail "the signature does not name the tag"
printf 'def  cineo-x86_64-linux\n' >"$work/dist/SHA256SUMS"
if minisign -V -q -p "$work/minisign.pub" -m "$work/dist/SHA256SUMS" -x "$work/dist/SHA256SUMS.minisig" 2>/dev/null; then
  fail "changed checksums still verify"
fi

if CINEO_MINISIGN_KEY=$(sed -n 2p "$work/secret.key") "$work/scripts/sign-release.sh" "$work/dist" v1.0.0 2>"$work/err" >/dev/null; then
  fail "a key without its comment line was accepted"
fi
grep -qF "whole secret key file" "$work/err" || fail "a key without its comment line was not explained"
if CINEO_MINISIGN_KEY=$(tr '\n' ' ' <"$work/secret.key") "$work/scripts/sign-release.sh" "$work/dist" v1.0.0 >/dev/null 2>&1; then
  fail "a key on one line was accepted"
fi

minisign -G -W -p "$work/other.pub" -s "$work/other.key" >/dev/null
printf 'abc  cineo-x86_64-linux\n' >"$work/dist/SHA256SUMS"
if CINEO_MINISIGN_KEY=$(cat "$work/other.key") "$work/scripts/sign-release.sh" "$work/dist" v1.0.0 >/dev/null 2>&1; then
  fail "a key other than minisign.pub's was accepted"
fi

((failures == 0)) || exit 1
echo "sign-release.sh: all cases passed"
