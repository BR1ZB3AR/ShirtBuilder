#!/usr/bin/env bash
# Put ShirtBuilder (Rust edition) in the app launcher for the current user.
#
#   packaging/install-desktop.sh              build (release) and install / update
#   packaging/install-desktop.sh --uninstall  remove it again
#
# Installs (no root needed):
#   ~/.local/bin/shirtbuilder
#   ~/.local/share/applications/shirtbuilder.desktop
#   ~/.local/share/icons/hicolor/256x256/apps/shirtbuilder.png
#   ~/.local/share/mime/packages/shirtbuilder.xml   (.sb design files open in ShirtBuilder)
# Re-run after every update to put the new version in the launcher.
set -euo pipefail
cd "$(dirname "$0")/.."

DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN="$HOME/.local/bin/shirtbuilder"
DESKTOP="$DATA/applications/shirtbuilder.desktop"
ICON="$DATA/icons/hicolor/256x256/apps/shirtbuilder.png"
MIME="$DATA/mime/packages/shirtbuilder.xml"
MIME_ICON="$DATA/icons/hicolor/256x256/mimetypes/application-x-shirtbuilder.png"

refresh() {
  command -v update-mime-database >/dev/null && update-mime-database "$DATA/mime" >/dev/null 2>&1 || true
  command -v update-desktop-database >/dev/null && update-desktop-database -q "$DATA/applications" || true
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$DATA/icons/hicolor" || true
  command -v kbuildsycoca6 >/dev/null && kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
}

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$BIN" "$DESKTOP" "$ICON" "$MIME" "$MIME_ICON"
  refresh
  echo "ShirtBuilder removed from the app launcher"
  exit 0
fi

VERSION=$(grep -m1 '^version' crates/shirtbuilder/Cargo.toml | cut -d'"' -f2)
echo "building ShirtBuilder $VERSION (release)"
cargo build --release -p shirtbuilder

mkdir -p "$(dirname "$BIN")" "$(dirname "$DESKTOP")" "$(dirname "$ICON")"
# copy to a temp name first so a running ShirtBuilder isn't overwritten in place
install -m755 target/release/shirtbuilder "$BIN.new"
strip "$BIN.new" 2>/dev/null || true
mv -f "$BIN.new" "$BIN"
install -m644 packaging/shirtbuilder.png "$ICON"
# .sb design files: their own file type with the ShirtBuilder icon, opened by ShirtBuilder
mkdir -p "$(dirname "$MIME")" "$(dirname "$MIME_ICON")"
install -m644 packaging/shirtbuilder-mime.xml "$MIME"
install -m644 packaging/shirtbuilder.png "$MIME_ICON"
# the packaged entry, with absolute paths so it works even when ~/.local/bin isn't on PATH
sed -e "s|^Exec=.*|Exec=\"$BIN\" %F|" \
    -e "s|^Icon=.*|Icon=$ICON|" \
    -e "/^Terminal=/a StartupNotify=true" \
    -e "/^Terminal=/a X-AppVersion=$VERSION" \
    packaging/shirtbuilder.desktop > "$DESKTOP.new"
chmod 644 "$DESKTOP.new"
mv -f "$DESKTOP.new" "$DESKTOP"
command -v desktop-file-validate >/dev/null && desktop-file-validate "$DESKTOP" || true
refresh
command -v xdg-mime >/dev/null && xdg-mime default shirtbuilder.desktop application/x-shirtbuilder || true

echo "ShirtBuilder $VERSION is in the app launcher ($DESKTOP); .sb designs open in it"
