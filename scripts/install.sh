#!/bin/sh
set -eu

repo=mdmrk/cineo
version=${CINEO_VERSION:-latest}
bin_dir=${XDG_BIN_HOME:-$HOME/.local/bin}
data_home=${XDG_DATA_HOME:-$HOME/.local/share}
exe=$bin_dir/cineo-desktop
desktop=$data_home/applications/cineo.desktop
icon=$data_home/icons/hicolor/256x256/apps/cineo.png
licenses=$data_home/doc/cineo

say() { printf '%s\n' "$*"; }
die() { say "error: $*" >&2; exit 1; }

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$exe" "$desktop" "$icon"
  rm -rf "$licenses"
  say "Cineo removed. Your addons and library stay in $data_home/cineo."
  exit 0
fi

[ "$(uname -s)" = Linux ] || die "this script is for Linux; see https://github.com/$repo/releases"
[ "$(uname -m)" = x86_64 ] || die "no build for $(uname -m) yet"
command -v curl >/dev/null || die "curl is required"
command -v sha256sum >/dev/null || die "sha256sum is required"

if [ -n "${CINEO_DOWNLOAD_URL:-}" ]; then
  base=$CINEO_DOWNLOAD_URL
elif [ "$version" = latest ]; then
  base=https://github.com/$repo/releases/latest/download
else
  base=https://github.com/$repo/releases/download/$version
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

say "Downloading Cineo ($version)..."
for file in cineo-x86_64-linux cineo.png cineo-licenses.zip SHA256SUMS; do
  curl -fsSL --retry 3 -o "$tmp/$file" "$base/$file" || die "could not download $base/$file"
done
(cd "$tmp" && grep -E ' \*?(cineo-x86_64-linux|cineo\.png|cineo-licenses\.zip)$' SHA256SUMS | sha256sum -c --quiet -) \
  || die "checksum mismatch; nothing was installed"

mkdir -p "$bin_dir" "$(dirname "$desktop")" "$(dirname "$icon")"
install -m 755 "$tmp/cineo-x86_64-linux" "$exe.new"
mv -f "$exe.new" "$exe"
install -m 644 "$tmp/cineo.png" "$icon"
if command -v unzip >/dev/null; then
  rm -rf "$licenses"
  mkdir -p "$licenses"
  unzip -qj "$tmp/cineo-licenses.zip" -d "$licenses"
fi
cat > "$desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Cineo
Comment=Media center for Stremio-protocol addons
Exec="$exe"
Icon=cineo
Terminal=false
Categories=AudioVideo;Video;Player;
EOF
command -v update-desktop-database >/dev/null && update-desktop-database -q "$(dirname "$desktop")" || true

say "Installed $exe"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) say "Note: $bin_dir is not in your PATH; start Cineo from your app menu or add it." ;;
esac

has_libmpv() {
  ldconfig -p 2>/dev/null | grep -q 'libmpv\.so\.2' && return 0
  for lib in /usr/lib/libmpv.so.2 /usr/lib64/libmpv.so.2 /usr/lib/x86_64-linux-gnu/libmpv.so.2; do
    [ -e "$lib" ] && return 0
  done
  return 1
}

if ! has_libmpv; then
  say ""
  say "Cineo plays video with libmpv, which was not found. Install it:"
  say "  Arch:          sudo pacman -S mpv"
  say "  Debian/Ubuntu: sudo apt install libmpv2"
  say "  Fedora:        sudo dnf install mpv-libs"
fi
