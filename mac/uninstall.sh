#!/bin/sh
# Remove the input method. Also remove it from System Settings > Keyboard > Input Sources.
set -eu
killall KhmerAutocomplete 2>/dev/null || true
rm -rf "$HOME/Library/Input Methods/KhmerAutocomplete.app"
defaults delete org.khmerautocomplete.inputmethod.KhmerAutocomplete 2>/dev/null || true
echo "Removed. Log out and back in to clear it from the input source list."
