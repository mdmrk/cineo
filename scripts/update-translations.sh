#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/common.sh
dir=crates/cineo-desktop/assets/i18n
potfiles=$(find crates/cineo-desktop/src -name '*.rs' | LC_ALL=C sort)
if [[ " $* " == *" --check "* ]]; then
  [[ $potfiles == "$(cat "$dir/POTFILES")" ]] || die "$dir/POTFILES is out of date; run scripts/update-translations.sh"
else
  printf '%s\n' "$potfiles" >"$dir/POTFILES"
fi
crate=$(crate_dir fastframe-i18n)
exec "$crate/scripts/update-translations.sh" --package Cineo --domain cineo \
  --bugs 'https://github.com/mdmrk/cineo/issues' --dir "$dir" \
  --keyword 't!:1' --keyword 'pt!:1c,2' "$@"
