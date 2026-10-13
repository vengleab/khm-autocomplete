#!/bin/sh
# Build mac/build/KhmerAutocomplete.app (universal arm64 + x86_64). Needs macOS, Xcode command line tools, rustup.
#   BUNDLE_ID=...   bundle identifier (default below; change it to your own reverse-DNS name)
#   VERSION=...     version string
#   SIGN_IDENTITY=  "Developer ID Application: Name (TEAMID)"; default "-" = ad-hoc signature (local use only)
#   ARCHS=...       default "arm64 x86_64"
set -eu
cd "$(dirname "$0")"
ROOT=$(cd .. && pwd)
BUNDLE_ID=${BUNDLE_ID:-org.khmerautocomplete.inputmethod.KhmerAutocomplete}
VERSION=${VERSION:-0.1.0}
ARCHS=${ARCHS:-"arm64 x86_64"}
IDENTITY=${SIGN_IDENTITY:--}
APP=build/KhmerAutocomplete.app

[ "$(uname)" = Darwin ] || { echo "mac/build.sh must run on macOS" >&2; exit 1; }
mkdir -p lib build

echo "== 1/5 Rust engine (static library per architecture, then universal)"
LIBS=""
SWIFT_ARCHS=""
for a in $ARCHS; do
  case "$a" in
    arm64) T=aarch64-apple-darwin ;;
    x86_64) T=x86_64-apple-darwin ;;
    *) echo "unknown arch $a" >&2; exit 1 ;;
  esac
  rustup target add "$T"
  (cd "$ROOT" && cargo build --release -p khcore-capi --target "$T")
  LIBS="$LIBS $ROOT/target/$T/release/libkhcore_capi.a"
  SWIFT_ARCHS="$SWIFT_ARCHS --arch $a"
done
# shellcheck disable=SC2086
lipo -create $LIBS -output lib/libkhcore_capi.a

echo "== 2/5 compile the language data"
(cd "$ROOT" && cargo run --release -q -p khcore --bin khbuild -- data/data.json data/lexicon.json mac/build/khdata.bin data/english.json)

echo "== 3/5 Swift"
# shellcheck disable=SC2086
swift build -c release $SWIFT_ARCHS -Xlinker -L"$PWD/lib"
# shellcheck disable=SC2086
BIN=$(swift build -c release $SWIFT_ARCHS --show-bin-path)/KhmerAutocomplete

echo "== 4/5 assemble the app bundle"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/KhmerAutocomplete"
cp build/khdata.bin "$APP/Contents/Resources/khdata.bin"
sed -e "s|@BUNDLE_ID@|$BUNDLE_ID|g" -e "s|@VERSION@|$VERSION|g" Resources/Info.plist > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"

echo "== 5/5 sign ($IDENTITY)"
if [ "$IDENTITY" = "-" ]; then
  codesign --force --sign - "$APP"
else
  # hardened runtime + secure timestamp are required for notarization (see RELEASE.md)
  codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
fi
codesign --verify --strict --verbose=2 "$APP"
echo "built $APP"
