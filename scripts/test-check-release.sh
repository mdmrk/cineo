#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/scripts"
cp scripts/check-release.sh "$work/scripts/"
printf '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "2.3.4"\nedition = "2024"\n\n[workspace.metadata.android]\nversion = "9.9.9"\n' >"$work/Cargo.toml"
printf 'LIBMPV_FILE=mpv-dev-lgpl-x86_64-20260101-git-abc.7z\n' >"$work/scripts/libmpv-windows.env"

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

complete() {
  rm -rf "$work/dist"
  mkdir "$work/dist"
  (cd "$work/dist" && touch cineo-x86_64-linux cineo-x86_64-windows.exe cineo-arm64-android.apk \
    cineo-universal-macos.dmg mpv-dev-lgpl-x86_64-20260101-git-abc.7z cineo-licenses.zip cineo.png \
    install.sh install.ps1)
}

complete
expect pass "every release file" assets "$work/dist"
rm "$work/dist/cineo-universal-macos.dmg"
expect fail "a missing file" assets "$work/dist"
complete
touch "$work/dist/stray.txt"
expect fail "an extra file" assets "$work/dist"
complete
mkdir "$work/dist/cineo-licenses"
expect fail "an extra directory" assets "$work/dist"
complete
mv "$work/dist/mpv-dev-lgpl-x86_64-20260101-git-abc.7z" "$work/dist/mpv-dev-lgpl-x86_64-20251201-git-def.7z"
expect fail "a libmpv archive other than the pinned one" assets "$work/dist"
expect fail "a missing directory" assets "$work/nothing"

printf '[workspace.package]\nversion = "2.3.4"\n' >"$work/Cargo.toml"
expect fail "a version without notes" notes
mkdir -p "$work/docs/releases"
printf ' \n\n' >"$work/docs/releases/v2.3.4.md"
expect fail "empty notes" notes
printf -- '- Fixed a thing.\n' >"$work/docs/releases/v2.3.4.md"
expect pass "written notes" notes
printf -- '- Older.\n' >"$work/docs/releases/v2.3.3.md"
rm "$work/docs/releases/v2.3.4.md"
expect fail "notes of another version only" notes

cp scripts/release-notes.sh "$work/scripts/"
printf -- '- Fixed a thing.\n' >"$work/docs/releases/v2.3.4.md"
body=$(GITHUB_REPOSITORY=owner/cineo "$work/scripts/release-notes.sh" v2.3.4 v2.3.3)
for want in \
  "https://github.com/owner/cineo/releases/download/v2.3.4/cineo-x86_64-linux" \
  "https://github.com/owner/cineo/releases/download/v2.3.4/cineo-x86_64-windows.exe" \
  "https://github.com/owner/cineo/releases/download/v2.3.4/cineo-universal-macos.dmg" \
  "https://github.com/owner/cineo/releases/download/v2.3.4/cineo-arm64-android.apk" \
  "- Fixed a thing." \
  "**Full changelog**: https://github.com/owner/cineo/compare/v2.3.3...v2.3.4"; do
  grep -qF -- "$want" <<<"$body" || {
    echo "FAIL: the release body has no $want" >&2
    failures=$((failures + 1))
  }
done
if GITHUB_REPOSITORY=owner/cineo "$work/scripts/release-notes.sh" v9.9.9 >/dev/null 2>&1; then
  echo "FAIL: a body was written without notes" >&2
  failures=$((failures + 1))
fi
grep -q "Full changelog" <<<"$(GITHUB_REPOSITORY=owner/cineo "$work/scripts/release-notes.sh" v2.3.4)" && {
  echo "FAIL: a first release links a changelog" >&2
  failures=$((failures + 1))
}

printf 'untrusted comment: minisign public key\nRWSkey1\n' >"$work/minisign.pub"
printf 'repo=mdmrk/cineo\nminisign_key=RWSkey1\n' >"$work/scripts/install.sh"
expect pass "install.sh holding minisign.pub's key" key
printf 'repo=mdmrk/cineo\nminisign_key=RWSkey2\n' >"$work/scripts/install.sh"
expect fail "install.sh holding another key" key
rm "$work/minisign.pub"
expect fail "no minisign.pub" key

((failures == 0)) || exit 1
echo "check-release.sh: all cases passed"
