#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
sh scripts/bundle.sh
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(lipo -archs dist/Atelier.app/Contents/MacOS/atelier-terminal)
DMG="dist/Atelier-${VERSION}-macOS-${ARCH}.dmg"
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/atelier-dmg.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT HUP INT TERM
mkdir "$STAGE/payload"
ditto dist/Atelier.app "$STAGE/payload/Atelier.app"
ln -s /Applications "$STAGE/payload/Applications"
hdiutil makehybrid -hfs -hfs-volume-name Atelier -o "$STAGE/source.dmg" "$STAGE/payload"
hdiutil convert "$STAGE/source.dmg" -ov -format UDZO -o "$DMG"
hdiutil verify "$DMG"
codesign --verify --deep --strict dist/Atelier.app
(cd dist && shasum -a 256 "$(basename "$DMG")") > "$DMG.sha256"
printf 'DMG : %s/%s\n' "$PWD" "$DMG"
