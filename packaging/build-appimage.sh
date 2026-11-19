#!/usr/bin/env bash
# Build ShirtBuilder (Rust edition) as an AppImage: target/appimage/ShirtBuilder-<version>-x86_64.AppImage
#
#   SKIP_BUILD=1 ./packaging/build-appimage.sh   pack an existing target/release/shirtbuilder
#
# Needs the official AppImage tools (downloaded once into $APPIMAGE_TOOLS_DIR,
# default ~/.cache/shirtbuilder/appimage-tools):
#   appimagetool-x86_64.AppImage  — https://github.com/AppImage/appimagetool/releases
#   runtime-x86_64                — https://github.com/AppImage/type2-runtime/releases
set -euo pipefail
cd "$(dirname "$0")/.."

TOOLS="${APPIMAGE_TOOLS_DIR:-$HOME/.cache/shirtbuilder/appimage-tools}"
VERSION=$(grep -m1 '^version' crates/shirtbuilder/Cargo.toml | cut -d'"' -f2)
OUT=target/appimage
APPDIR="$OUT/ShirtBuilder.AppDir"
IMAGE="$OUT/ShirtBuilder-$VERSION-x86_64.AppImage"

mkdir -p "$TOOLS"
fetch() { # name url
  if [ ! -s "$TOOLS/$1" ]; then
    echo "downloading $1"
    curl -fL --retry 3 -o "$TOOLS/$1.part" "$2" && mv "$TOOLS/$1.part" "$TOOLS/$1"
  fi
  chmod +x "$TOOLS/$1"
}
fetch appimagetool-x86_64.AppImage https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
fetch runtime-x86_64 https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64

# SKIP_BUILD=1 packs an already built target/release/shirtbuilder (the release workflow builds it
# inside an older distribution, so the AppImage also runs where glibc is older than here)
if [ -z "${SKIP_BUILD:-}" ]; then
  echo "building ShirtBuilder $VERSION (release)"
  cargo build --release -p shirtbuilder
fi
test -x target/release/shirtbuilder || { echo "no target/release/shirtbuilder to pack" >&2; exit 1; }

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps" "$APPDIR/usr/share/metainfo" "$APPDIR/usr/share/mime/packages"
install -m755 target/release/shirtbuilder "$APPDIR/usr/bin/shirtbuilder"
strip --strip-unneeded "$APPDIR/usr/bin/shirtbuilder" 2>/dev/null || true
install -m644 packaging/shirtbuilder.desktop "$APPDIR/usr/share/applications/shirtbuilder.desktop"
install -m644 packaging/shirtbuilder.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/shirtbuilder.png"
# the .sb design-file type, for AppImage integration tools (appimaged, Gear Lever, AppImageLauncher)
install -m644 packaging/shirtbuilder-mime.xml "$APPDIR/usr/share/mime/packages/shirtbuilder.xml"
sed -e "s/@VERSION@/$VERSION/" -e "s/@DATE@/$(date +%F)/" packaging/shirtbuilder.metainfo.xml > "$APPDIR/usr/share/metainfo/shirtbuilder.appdata.xml"
cp packaging/shirtbuilder.desktop "$APPDIR/shirtbuilder.desktop"
cp packaging/shirtbuilder.png "$APPDIR/shirtbuilder.png"
ln -sf shirtbuilder.png "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
# AppImage entry point
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/shirtbuilder" "$@"
RUN
chmod +x "$APPDIR/AppRun"

# graphics, windowing and keyboard libraries (Vulkan/GL drivers, Wayland, X11, xkbcommon) are
# loaded from the host at run time, as they must be for GPU drivers — nothing else to bundle
echo "packing $IMAGE"
ARCH=x86_64 "$TOOLS/appimagetool-x86_64.AppImage" --appimage-extract-and-run --no-appstream \
  --runtime-file "$TOOLS/runtime-x86_64" "$APPDIR" "$IMAGE"
chmod +x "$IMAGE"
ls -lh "$IMAGE"
