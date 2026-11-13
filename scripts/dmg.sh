#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
sh scripts/bundle.sh
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(lipo -archs dist/Atelier.app/Contents/MacOS/atelier-terminal)
DMG="dist/Atelier-${VERSION}-macOS-${ARCH}.dmg"
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/atelier-dmg.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT HUP INT TERM
ditto dist/Atelier.app "$STAGE/Atelier.app"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname Atelier -srcfolder "$STAGE" -ov -format UDZO "$DMG"
hdiutil verify "$DMG"
codesign --verify --deep --strict dist/Atelier.app
(cd dist && shasum -a 256 "$(basename "$DMG")") > "$DMG.sha256"
printf 'DMG : %s/%s\n' "$PWD" "$DMG"
