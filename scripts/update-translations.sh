#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
dir=crates/cineo-desktop/assets/i18n
find crates/cineo-desktop/src -name '*.rs' | LC_ALL=C sort >"$dir/POTFILES"
crate=$(cargo metadata --format-version 1 --locked |
    grep -o '"manifest_path":"[^"]*fastframe-i18n/Cargo.toml"' | head -n1 |
    sed 's/^"manifest_path":"//; s/Cargo.toml"$//')
exec "$crate/scripts/update-translations.sh" --package Cineo --domain cineo \
    --bugs 'https://github.com/mdmrk/cineo/issues' --dir "$dir" \
    --keyword 't!:1' --keyword 'pt!:1c,2' "$@"
