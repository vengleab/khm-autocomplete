# Releasing the macOS input method (notes; nothing here has been run)

No Apple accounts or secrets are set up by this repository. These are the steps to do once you have them.

## 1. Sign (Developer ID)
Needs an Apple Developer Program membership and a **Developer ID Application** certificate in your keychain.
```
SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" BUNDLE_ID=com.yourcompany.inputmethod.KhmerAutocomplete mac/build.sh
```
`build.sh` signs with the hardened runtime and a secure timestamp (both are required for notarization).
No entitlements are needed: the app makes no network calls, uses no camera, microphone or files outside its own bundle
and preferences. Do not turn on the App Sandbox unless you also add the `com.apple.security.temporary-exception.mach-lookup.global-name`
entitlement for the connection name in `Info.plist` (`<bundle id>_Connection`); sandboxed input methods need that exception.

## 2. Notarize and staple
```
ditto -c -k --keepParent mac/build/KhmerAutocomplete.app mac/build/KhmerAutocomplete.zip
xcrun notarytool store-credentials khmer-notary --apple-id you@example.com --team-id TEAMID    # once; app-specific password
xcrun notarytool submit mac/build/KhmerAutocomplete.zip --keychain-profile khmer-notary --wait
xcrun stapler staple mac/build/KhmerAutocomplete.app
spctl --assess --type execute --verbose mac/build/KhmerAutocomplete.app
```
If notarization fails, `xcrun notarytool log <submission-id> --keychain-profile khmer-notary` says why (usually a missing
hardened runtime or an unsigned nested binary).

## 3. Installer
An input method must live in `/Library/Input Methods` (all users) or `~/Library/Input Methods` (one user).
A signed installer package is the usual way:
```
pkgbuild --component mac/build/KhmerAutocomplete.app --install-location "/Library/Input Methods" mac/build/KhmerAutocomplete-unsigned.pkg
productsign --sign "Developer ID Installer: Your Name (TEAMID)" mac/build/KhmerAutocomplete-unsigned.pkg mac/build/KhmerAutocomplete.pkg
xcrun notarytool submit mac/build/KhmerAutocomplete.pkg --keychain-profile khmer-notary --wait && xcrun stapler staple mac/build/KhmerAutocomplete.pkg
```
A postinstall script should `killall KhmerAutocomplete` so the new version loads. The user still adds the input source once
in System Settings (macOS does not allow an installer to enable an input source silently).

## 4. Updates with Sparkle (plan only)
1. Add Sparkle 2 as a Swift package dependency of the app target; set `SUFeedURL` (an HTTPS appcast URL you host) and
   `SUPublicEDKey` in `Info.plist`.
2. Generate the EdDSA key pair once with Sparkle's `generate_keys`; keep the private key in the keychain / a secret store.
   Sign each release archive with `sign_update` and publish an `appcast.xml` (`generate_appcast`).
3. Start an `SPUStandardUpdaterController` from `main.swift`. The app is an accessory app, so the standard update dialog
   works, but check once that it appears in front of other apps.
4. Updating a running input method replaces its bundle on disk; the new version loads the next time the input source is
   activated (Sparkle relaunches the app; macOS restarts the input method process on demand).
5. Updates are the *only* network use in the whole product. Keep it off by default or opt-in if the "no network" promise
   must hold literally; the typing engine itself never touches the network, and typed text is never sent anywhere.

## 5. Checklist before shipping
- Replace `BUNDLE_ID` and the copyright line in `Resources/Info.plist`; add the icon files.
- Run `python3 mac/check-plist.py` and the bridge tests (`swift test`).
- Try it in: TextEdit, Safari, Chrome, Notes, Terminal, VS Code, a password field (must stay off), Spotlight.
