#!/usr/bin/env python3
"""Check mac/Resources/Info.plist: parses, has the keys InputMethodKit needs, and the placeholders are consistent.
Runs anywhere (no macOS needed):  python3 mac/check-plist.py [Info.plist]"""
import plistlib, sys, os

path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "Resources", "Info.plist")
raw = open(path, "rb").read()
d = plistlib.loads(raw)
errors = []
def need(cond, msg):
    if not cond: errors.append(msg)

for k in ["CFBundleExecutable", "CFBundleIdentifier", "CFBundlePackageType", "CFBundleShortVersionString", "CFBundleVersion",
          "InputMethodConnectionName", "InputMethodServerControllerClass", "ComponentInputModeDict", "LSUIElement"]:
    need(k in d, f"missing {k}")
need(d.get("CFBundlePackageType") == "APPL", "CFBundlePackageType must be APPL")
need(d.get("LSUIElement") is True, "LSUIElement must be true (an input method has no Dock icon)")
bid = d.get("CFBundleIdentifier", "")
need(d.get("InputMethodConnectionName", "").startswith(bid) and bid != "", "connection name should start with the bundle identifier")
need(d.get("InputMethodServerControllerClass") == "KhmerInputController", "controller class must match @objc(KhmerInputController)")
# the controller class name must exist in the sources under that @objc name
src = open(os.path.join(os.path.dirname(path), "..", "Sources", "KhmerAutocomplete", "InputController.swift"), encoding="utf-8").read()
need("@objc(KhmerInputController)" in src, "InputController.swift does not declare @objc(KhmerInputController)")
modes = d.get("ComponentInputModeDict", {})
listed = modes.get("tsInputModeListKey", {})
need(len(listed) >= 1, "no input mode listed")
order = modes.get("tsVisibleInputModeOrderedArrayKey", [])
need(set(order) == set(listed), "tsVisibleInputModeOrderedArrayKey must list exactly the modes")
for mid, m in listed.items():
    need(m.get("TISInputSourceID") == mid, f"{mid}: TISInputSourceID must equal the mode id")
    need(m.get("TISIntendedLanguage") == "km", f"{mid}: intended language must be km")
    need("tsInputModeScriptKey" in m, f"{mid}: missing tsInputModeScriptKey")
    need(mid.startswith(bid + "."), f"{mid}: mode id should extend the bundle identifier")
placeholders = {p for p in ("@BUNDLE_ID@", "@VERSION@") if p.encode() in raw}
need(placeholders == {"@BUNDLE_ID@", "@VERSION@"}, "template placeholders @BUNDLE_ID@ / @VERSION@ must both be present")
if errors:
    print("Info.plist problems:\n  " + "\n  ".join(errors)); sys.exit(1)
print("Info.plist OK:", path)
