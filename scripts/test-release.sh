#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/common.sh
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failures=0
fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}
checkout() {
  mkdir -p "$1/scripts"
  cp scripts/release.sh scripts/common.sh "$1/scripts/"
}

repo=$work/repo
checkout "$repo"
expect() {
  local outcome=$1 what=$2
  shift 2
  if "$repo/scripts/release.sh" "$@" >/dev/null 2>&1; then
    [[ $outcome == pass ]] || fail "$what was accepted"
  else
    [[ $outcome == fail ]] || fail "$what was refused"
  fi
}
printf '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "2.3.4"\nedition = "2024"\n\n[workspace.metadata.android]\nversion = "9.9.9"\n' >"$repo/Cargo.toml"
printf 'LIBMPV_FILE=mpv-dev-lgpl-x86_64-20260101-git-abc.7z\n' >"$repo/scripts/libmpv-windows.env"
expect pass "a tag naming the workspace version" version v2.3.4
expect fail "a tag naming another version" version v2.3.5
expect fail "a tag naming the Android metadata version" version v9.9.9
expect fail "a prerelease tag of a plain version" version v2.3.4-rc.1
printf '[workspace.package]\nversion = "2.3.4-rc.1"\n' >"$repo/Cargo.toml"
expect pass "a prerelease tag naming the workspace version" version v2.3.4-rc.1
expect fail "a plain tag of a prerelease version" version v2.3.4
expect fail "no tag" version
expect fail "an unknown command" nothing
mkdir -p "$work/fake/minisign-linux/x86_64"
printf '#!/bin/sh\n' >"$work/fake/minisign-linux/x86_64/minisign"
tar -czf "$work/fake.tar.gz" -C "$work/fake" minisign-linux
CINEO_MINISIGN_URL=file://$work/fake.tar.gz expect fail "a minisign archive other than the pinned one" tool "$work/tool"
[[ ! -e $work/tool/minisign ]] || fail "a minisign from another archive was installed"
complete() {
  rm -rf "$repo/dist"
  mkdir "$repo/dist"
  (cd "$repo/dist" && touch cineo-x86_64-linux cineo-x86_64-windows.exe cineo-arm64-android.apk \
    cineo-universal-macos.dmg mpv-dev-lgpl-x86_64-20260101-git-abc.7z cineo-licenses.zip cineo.png \
    install.sh install.ps1)
}
complete
expect pass "every release file" assets "$repo/dist"
rm "$repo/dist/cineo-universal-macos.dmg"
expect fail "a missing file" assets "$repo/dist"
complete
touch "$repo/dist/stray.txt"
expect fail "an extra file" assets "$repo/dist"
complete
mkdir "$repo/dist/cineo-licenses"
expect fail "an extra directory" assets "$repo/dist"
complete
mv "$repo/dist/mpv-dev-lgpl-x86_64-20260101-git-abc.7z" "$repo/dist/mpv-dev-lgpl-x86_64-20251201-git-def.7z"
expect fail "a libmpv archive other than the pinned one" assets "$repo/dist"
expect fail "a missing directory" assets "$repo/nothing"
printf '[workspace.package]\nversion = "2.3.4"\n' >"$repo/Cargo.toml"
expect fail "a version without notes" notes
mkdir -p "$repo/docs/releases"
printf ' \n\n' >"$repo/docs/releases/v2.3.4.md"
expect fail "empty notes" notes
printf -- '- Fixed a thing.\n' >"$repo/docs/releases/v2.3.4.md"
expect pass "written notes" notes
printf -- '- Older.\n' >"$repo/docs/releases/v2.3.3.md"
rm "$repo/docs/releases/v2.3.4.md"
expect fail "notes of another version only" notes
printf -- '- Fixed a thing.\n' >"$repo/docs/releases/v2.3.4.md"
body=$(GITHUB_REPOSITORY=owner/cineo "$repo/scripts/release.sh" body v2.3.4 v2.3.3)
for want in \
  "https://github.com/owner/cineo/releases/download/v2.3.4/cineo-x86_64-linux" \
  "https://github.com/owner/cineo/releases/download/v2.3.4/cineo-x86_64-windows.exe" \
  "https://github.com/owner/cineo/releases/download/v2.3.4/cineo-universal-macos.dmg" \
  "https://github.com/owner/cineo/releases/download/v2.3.4/cineo-arm64-android.apk" \
  "- Fixed a thing." \
  "**Full changelog**: https://github.com/owner/cineo/compare/v2.3.3...v2.3.4"; do
  grep -qF -- "$want" <<<"$body" || fail "the release body has no $want"
done
expect fail "a body without notes" body v9.9.9
grep -q "Full changelog" <<<"$(GITHUB_REPOSITORY=owner/cineo "$repo/scripts/release.sh" body v2.3.4)" &&
  fail "a first release links a changelog"
mkdir -p "$repo/crates/cineo-desktop/src"
updater() { printf 'pub const PUBLIC_KEY: &str = "%s";\n' "$1" >"$repo/crates/cineo-desktop/src/self_update.rs"; }
printf 'untrusted comment: minisign public key\nRWSkey1\n' >"$repo/minisign.pub"
printf 'repo=mdmrk/cineo\nminisign_key=RWSkey1\n' >"$repo/scripts/install.sh"
updater RWSkey1
expect pass "install.sh and the updater holding minisign.pub's key" key
updater RWSkey2
expect fail "the updater holding another key" key
updater RWSkey1
printf 'repo=mdmrk/cineo\nminisign_key=RWSkey2\n' >"$repo/scripts/install.sh"
expect fail "install.sh holding another key" key
rm "$repo/minisign.pub"
expect fail "no minisign.pub" key

if need minisign "your package manager"; then
  repo=$work/sign
  checkout "$repo"
  mkdir "$repo/dist"
  minisign -G -W -p "$repo/minisign.pub" -s "$work/secret.key" >/dev/null
  minisign -G -W -p "$work/other.pub" -s "$work/other.key" >/dev/null
  sums=$repo/dist/SHA256SUMS
  verify() { minisign -V -q -p "$repo/minisign.pub" -m "$sums" -x "$sums.minisig" 2>/dev/null; }
  printf 'abc  cineo-x86_64-linux\n' >"$sums"
  CINEO_MINISIGN_KEY='' expect fail "signing without a key" sign "$repo/dist" v1.0.0
  [[ ! -e $sums.minisig ]] || fail "a signature was left without a key"
  CINEO_MINISIGN_KEY=$(cat "$work/secret.key") expect pass "signing with the key" sign "$repo/dist" v1.0.0
  verify || fail "the signature does not verify"
  grep -qF "trusted comment: cineo v1.0.0 SHA256SUMS" "$sums.minisig" || fail "the signature does not name the tag"
  printf 'def  cineo-x86_64-linux\n' >"$sums"
  verify && fail "changed checksums still verify"
  if CINEO_MINISIGN_KEY=$(sed -n 2p "$work/secret.key") "$repo/scripts/release.sh" sign "$repo/dist" v1.0.0 2>"$work/err" >/dev/null; then
    fail "a key without its comment line was accepted"
  fi
  grep -qF "whole secret key file" "$work/err" || fail "a key without its comment line was not explained"
  CINEO_MINISIGN_KEY=$(tr '\n' ' ' <"$work/secret.key") expect fail "a key on one line" sign "$repo/dist" v1.0.0
  printf 'abc  cineo-x86_64-linux\n' >"$sums"
  CINEO_MINISIGN_KEY=$(cat "$work/other.key") expect fail "a key other than minisign.pub's" sign "$repo/dist" v1.0.0

  inst=$work/install
  mkdir -p "$inst/bin"
  sed "s|^minisign_key=.*|minisign_key=$(sed -n 2p "$repo/minisign.pub")|" scripts/install.sh >"$inst/install.sh"
  cat >"$inst/bin/curl" <<'CURL'
#!/bin/sh
while [ $# -gt 1 ]; do
  [ "$1" = -o ] && out=$2
  shift
done
cp "$RELEASE/$(basename "$1")" "$out" 2>/dev/null
CURL
  chmod +x "$inst/bin/curl"
  release() {
    rm -rf "${inst:?}/release" "${inst:?}/home"
    mkdir -p "$inst/release" "$inst/home"
    printf '#!/bin/sh\n' >"$inst/release/cineo-x86_64-linux"
    printf 'png' >"$inst/release/cineo.png"
    (cd "$inst/release" && python3 -I -m zipfile -c cineo-licenses.zip cineo.png &&
      sha256sum -- * >SHA256SUMS)
  }
  sign_with() {
    minisign -S -s "$1" -m "$inst/release/SHA256SUMS" -x "$inst/release/SHA256SUMS.minisig" >/dev/null
  }
  expect_install() {
    local outcome=$1 what=$2
    shift 2
    if env HOME="$inst/home" XDG_BIN_HOME= XDG_DATA_HOME= XDG_STATE_HOME= RELEASE="$inst/release" \
      PATH="$inst/bin:$PATH" "$@" sh "$inst/install.sh" >/dev/null 2>&1; then
      [[ $outcome == pass && -x $inst/home/.local/bin/cineo ]] || fail "$what was installed"
    else
      [[ $outcome == fail && ! -e $inst/home/.local/bin/cineo ]] || fail "$what was not installed"
    fi
  }
  release
  sign_with "$work/secret.key"
  expect_install pass "a signed release"
  release
  sign_with "$work/secret.key"
  printf '0000000000000000000000000000000000000000000000000000000000000000  extra\n' >>"$inst/release/SHA256SUMS"
  expect_install fail "checksums changed after signing"
  release
  expect_install fail "an unsigned latest release"
  release
  expect_install pass "an unsigned release picked by version" CINEO_VERSION=v1.0.0
  release
  sign_with "$work/other.key"
  expect_install fail "a release signed by another key"
  release
  sign_with "$work/secret.key"
  printf 'tampered' >"$inst/release/cineo.png"
  expect_install fail "a file that does not match its checksum"
fi
((failures == 0)) || exit 1
echo "release.sh and install.sh: all cases passed"
