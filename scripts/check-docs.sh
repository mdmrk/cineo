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

exit "$status"
