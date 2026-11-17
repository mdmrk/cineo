#!/usr/bin/env bash
set -euo pipefail

usage="usage: scripts/check-fdroid-build.sh <commit or tag> <release>, as root in the fdroidserver buildserver image"
ref=${1:?$usage}
release=${2:?$usage}
repo=$(cd "$(dirname "$0")/.." && pwd)
app=io.github.mdmrk.cineo
work=/home/vagrant/fdroiddata
git=(git -c safe.directory="$repo" -C "$repo")
commit=$("${git[@]}" rev-parse "$ref^{commit}")
cargo=$("${git[@]}" show "$commit:Cargo.toml")
version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' <<<"$cargo")
code=$(sed -n '/^\[workspace.metadata.android\]/,/^\[/s/^version-code = \([0-9]*\)/\1/p' <<<"$cargo")

apt-get update -qq
apt-get install -y -qq sudo python3-venv git >/dev/null
python3 -m venv /opt/fdroidserver
/opt/fdroidserver/bin/pip install --quiet fdroidserver==2.4.5

rm -rf "$work"
mkdir -p "$work/metadata"
"${git[@]}" show "$commit:docs/fdroid/$app.yml" >"$work/metadata/$app.yml"
python3 - "$work/metadata/$app.yml" "$version" "$code" "$commit" "$release" <<'PY'
import re, sys
target, version, code, commit, release = sys.argv[1:]
recipe = open(target).read()
for key, value in [
    (r"  - versionName", version),
    (r"    versionCode", code),
    (r"    commit", commit),
    (r"CurrentVersion", version),
    (r"CurrentVersionCode", code),
    (r"Binaries", f"https://github.com/mdmrk/cineo/releases/download/{release}/cineo-arm64-android.apk"),
]:
    recipe, count = re.subn(rf"(?m)^{key}: .*$", f"{key}: {value}", recipe)
    if count != 1:
        sys.exit(f"the recipe must have exactly one {key.strip(' -')}")
open(target, "w").write(recipe)
PY
printf 'sdk_path: /opt/android-sdk\ngradle: /usr/local/bin/gradlew-fdroid\n' >"$work/config.yml"
chmod 600 "$work/config.yml"
git -C "$work" init --quiet
git -C "$work" add .
git -C "$work" -c user.name=cineo -c user.email=cineo@localhost commit --quiet -m "$app $version"
chown -R vagrant:vagrant "$work"

cd "$work"
runuser -u vagrant -- env HOME=/home/vagrant /opt/fdroidserver/bin/fdroid build --on-server --no-tarball "$app:$code" 2>&1 |
  tee /tmp/fdroid-build.log
grep -q "compared built binary to supplied reference binary successfully" /tmp/fdroid-build.log
