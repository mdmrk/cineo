#!/usr/bin/env bash
set -euo pipefail

tag=${1:?usage: scripts/check-fdroid-build.sh vX.Y.Z, as root in the fdroidserver buildserver image}
repo=$(cd "$(dirname "$0")/.." && pwd)
app=io.github.mdmrk.cineo
work=/home/vagrant/fdroiddata
version=${tag#v}
IFS=. read -r major minor patch <<<"$version"
code=$((major * 10000 + minor * 100 + patch))
commit=$(git -c safe.directory="$repo" -C "$repo" rev-parse "$tag^{commit}")

apt-get update -qq
apt-get install -y -qq sudo python3-venv git >/dev/null
python3 -m venv /opt/fdroidserver
/opt/fdroidserver/bin/pip install --quiet fdroidserver==2.4.5

rm -rf "$work"
mkdir -p "$work/metadata"
python3 - "$repo/docs/fdroid/$app.yml" "$work/metadata/$app.yml" "$version" "$code" "$commit" <<'PY'
import re, sys
source, target, version, code, commit = sys.argv[1:]
recipe = open(source).read()
if recipe.count("  - versionName:") != 1:
    sys.exit(f"{source} must hold exactly one build")
for key, value in [
    (r"  - versionName", version),
    (r"    versionCode", code),
    (r"    commit", commit),
    (r"CurrentVersion", version),
    (r"CurrentVersionCode", code),
]:
    recipe = re.sub(rf"(?m)^{key}: .*$", f"{key}: {value}", recipe)
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
