#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/appimage.env

image=$(realpath "${1:?usage: check-appimage.sh APPIMAGE}")
version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fail() {
  echo "$image: $*" >&2
  exit 1
}

(cd "$work" && "$image" --appimage-extract >/dev/null)
lib=$work/squashfs-root/usr/lib/cineo
[[ -x $lib/cineo-desktop && -f $lib/libmpv.so.2 ]] || fail "has no usr/lib/cineo/cineo-desktop and libmpv.so.2"

for elf in "$lib"/*; do
  missing=$(ldd "$elf" | grep 'not found' || true)
  [[ -z $missing ]] || fail "$(basename "$elf") cannot find: $missing"
done

reported=$(APPIMAGE_EXTRACT_AND_RUN=1 "$image" --version)
[[ $reported == "cineo-desktop $version" ]] || fail "reports '$reported', not cineo-desktop $version"

embedded=$("$image" --appimage-updateinformation)
[[ $embedded == "$APPIMAGE_UPDATE" ]] || fail "carries the update information '$embedded'"

python3 -I - "$lib/libmpv.so.2" <<'PY'
import ctypes, sys
mpv = ctypes.CDLL(sys.argv[1])
mpv.mpv_create.restype = ctypes.c_void_p
mpv.mpv_set_option_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
mpv.mpv_initialize.argtypes = [ctypes.c_void_p]
mpv.mpv_terminate_destroy.argtypes = [ctypes.c_void_p]
handle = mpv.mpv_create()
if not handle:
    sys.exit("mpv_create failed")
for name, value in ((b"vo", b"null"), (b"ao", b"null"), (b"config", b"no")):
    if mpv.mpv_set_option_string(handle, name, value) != 0:
        sys.exit(f"mpv rejected {name.decode()}={value.decode()}")
if mpv.mpv_initialize(handle) != 0:
    sys.exit("mpv_initialize failed")
mpv.mpv_terminate_destroy(handle)
PY

echo "$image: starts, finds every library and initializes libmpv"
