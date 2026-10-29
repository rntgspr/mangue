#!/usr/bin/env bash
# Builds the release mangue binary (X11 + Wayland) and packs it with its
# .desktop entry, .bpmn MIME type and hicolor icons into
# target/release/bundle/mangue-<version>-linux-x86_64.tar.gz. Needs ImageMagick.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
NAME="mangue-$VERSION-linux-x86_64"
BUNDLE="target/release/bundle"
STAGE="$BUNDLE/$NAME"
ICON_SRC="assets/app-icon/mangue Exports/mangue-iOS-Default-1024@1x.png"
MAGICK="$(command -v magick || command -v convert || true)"

if [ -z "$MAGICK" ]; then
  echo "error: ImageMagick (magick or convert) is required to generate icons" >&2
  exit 1
fi

cargo build --release

rm -rf "$STAGE" "$BUNDLE/$NAME.tar.gz"
mkdir -p "$STAGE/bin" "$STAGE/share/applications" "$STAGE/share/mime/packages"
cp target/release/mangue "$STAGE/bin/mangue"
cp assets/app/mangue.desktop "$STAGE/share/applications/mangue.desktop"
cp assets/app/mangue-bpmn.xml "$STAGE/share/mime/packages/mangue-bpmn.xml"
cp README.md "$STAGE/README.md"

for size in 48 128 256 512; do
  dir="$STAGE/share/icons/hicolor/${size}x${size}/apps"
  mkdir -p "$dir"
  "$MAGICK" "$ICON_SRC" -resize "${size}x${size}" "$dir/mangue.png"
done

tar -C "$BUNDLE" -czf "$BUNDLE/$NAME.tar.gz" "$NAME"

echo "Tarball ready: $ROOT/$BUNDLE/$NAME.tar.gz"
