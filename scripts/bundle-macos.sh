#!/usr/bin/env bash
# Builds a universal (arm64 + x86_64) Mangue.app with its icon, signs it and
# wraps it in target/release/bundle/Mangue-<version>.dmg. With
# APPLE_SIGNING_IDENTITY it signs with Developer ID (hardened runtime) and, with
# APPLE_TEAM_ID + APPLE_ID + APPLE_APP_PASSWORD, notarizes and staples the DMG;
# otherwise it signs ad-hoc.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
BUNDLE="target/release/bundle"
APP="$BUNDLE/Mangue.app"
DMG="$BUNDLE/Mangue-$VERSION.dmg"
ICON_SRC="assets/app-icon/mangue Exports/mangue-iOS-Default-1024@1x.png"
ICON_DOC="assets/app-icon/mangue.icon"
WORK="target/release/icon-macos"

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target add "$target" >/dev/null
  cargo build --release --target "$target"
done

rm -rf "$APP" "$DMG" "$WORK"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$WORK/Mangue.iconset" "$WORK/actool"
lipo -create -output "$APP/Contents/MacOS/mangue" \
  target/aarch64-apple-darwin/release/mangue \
  target/x86_64-apple-darwin/release/mangue

# The export is a full-bleed square; inset it to the 824px macOS icon grid.
sips -Z 824 "$ICON_SRC" --out "$WORK/grid.png" >/dev/null
sips --padToHeightWidth 1024 1024 "$WORK/grid.png" --out "$WORK/icon-1024.png" >/dev/null
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$WORK/icon-1024.png" --out "$WORK/Mangue.iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$WORK/icon-1024.png" --out "$WORK/Mangue.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$WORK/Mangue.iconset" -o "$APP/Contents/Resources/Mangue.icns"

ICON_NAME_KEY=""
if xcrun actool "$ICON_DOC" --compile "$WORK/actool" --platform macosx \
  --minimum-deployment-target 13.0 --app-icon mangue \
  --output-partial-info-plist "$WORK/actool.plist" >/dev/null 2>&1 \
  && [ -f "$WORK/actool/Assets.car" ]; then
  cp "$WORK/actool/Assets.car" "$APP/Contents/Resources/Assets.car"
  ICON_NAME_KEY="<key>CFBundleIconName</key><string>mangue</string>"
else
  echo "warning: actool cannot compile $ICON_DOC (needs Xcode 26+, older actool exits 0 without Assets.car); using Mangue.icns only" >&2
fi

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>io.mangue.app</string>
  <key>CFBundleName</key><string>Mangue</string>
  <key>CFBundleDisplayName</key><string>Mangue</string>
  <key>CFBundleExecutable</key><string>mangue</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleIconFile</key><string>Mangue</string>
  $ICON_NAME_KEY
  <key>NSPrincipalClass</key><string>NSApplication</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>BPMN Diagram</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>CFBundleTypeExtensions</key>
      <array><string>bpmn</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
if [ -n "$IDENTITY" ]; then
  codesign --force --options runtime --timestamp \
    --entitlements scripts/entitlements.plist -s "$IDENTITY" "$APP"
else
  echo "warning: APPLE_SIGNING_IDENTITY not set; ad-hoc signing (other Macs need right-click > Open)" >&2
  codesign --force --deep -s - "$APP"
fi

STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Mangue $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null

if [ -n "$IDENTITY" ]; then
  codesign --force --timestamp -s "$IDENTITY" "$DMG"
  if [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_APP_PASSWORD:-}" ]; then
    xcrun notarytool submit "$DMG" --wait \
      --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD"
    xcrun stapler staple "$DMG"
  else
    echo "warning: APPLE_TEAM_ID / APPLE_ID / APPLE_APP_PASSWORD not set; DMG is signed but not notarized" >&2
  fi
fi

echo "Bundle ready: $ROOT/$APP"
echo "Disk image ready: $ROOT/$DMG"
