#!/bin/sh
# Install the built app as the current user's input method, then add it in System Settings.
set -eu
cd "$(dirname "$0")"
APP=build/KhmerAutocomplete.app
DEST="$HOME/Library/Input Methods"
[ -d "$APP" ] || { echo "run mac/build.sh first" >&2; exit 1; }
mkdir -p "$DEST"
# stop a running copy so the new one is loaded
killall KhmerAutocomplete 2>/dev/null || true
rm -rf "$DEST/KhmerAutocomplete.app"
cp -R "$APP" "$DEST/"
echo "Installed to: $DEST/KhmerAutocomplete.app"
cat <<'MSG'

Next, once:
  1. System Settings > Keyboard > Text Input > Input Sources > Edit... > "+"
  2. Choose "Khmer" (or search "Khmer Autocomplete") and add it.
  3. If it is not listed, log out and back in (macOS scans for new input methods at login).
Switch to it from the input menu in the menu bar. Modes: Khmer script, Latin (Khmerlish), English -> Khmer;
change mode from the input menu or with Control + Option + Space.
MSG
