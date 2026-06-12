#!/bin/bash
# Build "Open Dungeon.app" and a distributable DMG (Apple Silicon).
# Usage: scripts/package-mac.sh <version> <icon-png>
set -euo pipefail

VERSION="${1:?usage: package-mac.sh <version> <icon-png>}"
ICON_PNG="${2:?usage: package-mac.sh <version> <icon-png>}"
REPO="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$REPO/dist"
CACHE="$DIST/cache"
APP="$DIST/Open Dungeon.app"
NODE_VERSION="v22.22.2"
NODE_TARBALL="node-$NODE_VERSION-darwin-arm64.tar.gz"

mkdir -p "$CACHE"
rm -rf "$APP" "$DIST/staging" "$DIST/Open-Dungeon-$VERSION.dmg"

# --- Node runtime (cached) ---
if [ ! -f "$CACHE/$NODE_TARBALL" ]; then
  echo "Downloading Node runtime $NODE_VERSION..."
  curl -fL --progress-bar "https://nodejs.org/dist/$NODE_VERSION/$NODE_TARBALL" -o "$CACHE/$NODE_TARBALL"
fi

# --- App bundle skeleton ---
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Open Dungeon</string>
  <key>CFBundleDisplayName</key><string>Open Dungeon</string>
  <key>CFBundleIdentifier</key><string>com.opendungeon.app</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>OpenDungeon</string>
  <key>CFBundleIconFile</key><string>app</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
</dict>
</plist>
PLIST

# Executable: run the setup script in Terminal so the user sees progress.
cat > "$APP/Contents/MacOS/OpenDungeon" <<'EXEC'
#!/bin/bash
RES="$(cd "$(dirname "$0")/../Resources" && pwd)"
open -a Terminal "$RES/setup-app.sh"
EXEC
chmod +x "$APP/Contents/MacOS/OpenDungeon"

# --- Resources ---
cp "$CACHE/$NODE_TARBALL" "$APP/Contents/Resources/node-runtime.tar.gz"
cp "$REPO/scripts/setup-app.sh" "$APP/Contents/Resources/setup-app.sh"
chmod +x "$APP/Contents/Resources/setup-app.sh"
echo "$VERSION" > "$APP/Contents/Resources/VERSION"
git -C "$REPO" archive --format=tar.gz HEAD -o "$APP/Contents/Resources/app-src.tar.gz"

# --- Icon ---
ICONSET="$DIST/app.iconset"
rm -rf "$ICONSET" && mkdir -p "$ICONSET"
for size in 16 32 64 128 256 512; do
  sips -z $size $size "$ICON_PNG" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) "$ICON_PNG" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/app.icns"
rm -rf "$ICONSET"

# --- Ad-hoc sign (no Developer ID; users right-click > Open the first time) ---
codesign --force --deep -s - "$APP"

# --- DMG ---
mkdir -p "$DIST/staging"
cp -R "$APP" "$DIST/staging/"
ln -s /Applications "$DIST/staging/Applications"
hdiutil create -volname "Open Dungeon" -srcfolder "$DIST/staging" -ov -format UDZO \
  "$DIST/Open-Dungeon-$VERSION.dmg" >/dev/null
rm -rf "$DIST/staging"

echo "Built: $DIST/Open-Dungeon-$VERSION.dmg"
du -h "$DIST/Open-Dungeon-$VERSION.dmg" | cut -f1
