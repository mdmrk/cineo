#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

status=0
docs=(docs/COMPATIBILITY.md docs/ARCHITECTURE.md docs/SECURITY.md docs/DEVELOPMENT.md docs/TECHNICAL_DEBT.md)

fail() {
  echo "$1" >&2
  status=1
}

identifiers=$(git ls-files -z 'crates/*.rs' | xargs -0 grep -ohE '[a-z0-9_]+' | sort -u)

while IFS= read -r name; do
  grep -Fxq "$name" <<<"$identifiers" || fail "docs name \`$name\`, which is not in the code"
done < <(grep -ohE '`[a-z0-9]+(_[a-z0-9]+){3,}`' "${docs[@]}" | tr -d '`' | sort -u)

table=$(sed -n '/^### In use/,/^### Deliberately/p' docs/DEVELOPMENT.md | grep '^|')

while IFS= read -r line; do
  name=${line%% =*}
  [[ $name == cineo-* ]] && continue
  rows=$(grep -F "\`$name\`" <<<"$table" || true)
  if [[ -z $rows ]]; then
    fail "DEVELOPMENT.md has no row for the dependency $name"
    continue
  fi
  version=$(sed -nE 's/^[^=]+= "([^"]+)".*/\1/p; s/.*[{ ]version = "([^"]+)".*/\1/p' <<<"$line")
  if [[ -n $version ]] && ! grep -qF "$version" <<<"$rows"; then
    fail "DEVELOPMENT.md does not list $name $version"
  fi
done < <(sed -n '/^\[workspace.dependencies\]/,/^\[workspace.lints/p' Cargo.toml | grep -E '^[a-z0-9_-]+ = ')

python3 -I - docs/fdroid/io.github.mdmrk.cineo.yml Cargo.toml scripts/libmpv-android.env scripts/android.env <<'PY' || status=1
import re, sys
recipe, cargo, libmpv_env, android_env = (open(path).read() for path in sys.argv[1:])
pins = {
    "NDK": (re.search(r"^NDK_VERSION=(.+)$", libmpv_env, re.M).group(1),
            re.findall(r"^    ndk: (.+)$", recipe, re.M)[-1]),
    "cargo-ndk": (re.search(r"^CARGO_NDK_VERSION=(.+)$", android_env, re.M).group(1),
                  re.findall(r"cargo install cargo-ndk --version (\S+)", recipe)[-1]),
}
for tool, (pinned, built) in pins.items():
    if pinned != built:
        print(f"{sys.argv[1]} builds with {tool} {built}, the scripts pin {pinned}", file=sys.stderr)
        sys.exit(1)
data = re.search(r"^UpdateCheckData: (.*)$", recipe, re.M).group(1)
code_file, code_re, version_file, version_re = data.split("|")
if (code_file, version_file) != ("Cargo.toml", "."):
    sys.exit(f"{sys.argv[1]}: UpdateCheckData must read Cargo.toml, not {data}")
code = re.search(code_re, cargo)
version = re.search(version_re, cargo)
if not code or not version:
    sys.exit(f"{sys.argv[1]}: UpdateCheckData does not match Cargo.toml")
package = re.search(r'^\[workspace\.package\]\nversion = "(\d+)\.(\d+)\.(\d+)"$', cargo, re.M)
major, minor, patch = map(int, package.groups())
if version.group(1) != ".".join(package.groups()):
    sys.exit(f"UpdateCheckData reads version {version.group(1)}, Cargo.toml has {'.'.join(package.groups())}")
if int(code.group(1)) != major * 10000 + minor * 100 + patch:
    sys.exit(f"Cargo.toml version-code {code.group(1)} is not MAJOR * 10000 + MINOR * 100 + PATCH")
PY

exit "$status"
