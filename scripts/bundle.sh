#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
sh scripts/cargo.sh build --release --locked
APP="dist/Atelier.app"
mkdir -p "$APP/Contents/MacOS"
BINARY="target/${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET/}release/atelier-terminal"
cp "$BINARY" "$APP/Contents/MacOS/atelier-terminal"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Atelier</string>
<key>CFBundleDisplayName</key><string>Atelier</string>
<key>CFBundleIdentifier</key><string>local.atelier.terminal</string>
<key>CFBundleExecutable</key><string>atelier-terminal</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>${MACOSX_DEPLOYMENT_TARGET}</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --deep --sign - "$APP"
printf 'Application : %s/%s\n' "$PWD" "$APP"
