# macOS input method

A thin InputMethodKit app over the Rust engine. Keys and the text before the caret go in, candidates come out,
the chosen word is inserted. All language logic is in `core/` (called through the C API, `capi/include/khcore.h`).

```
Sources/CKhCore/            module map for the C header (khcore.h is a symlink to capi/include/khcore.h)
Sources/KhBridge/           Swift wrapper over the C API: no AppKit, no language logic (also builds on Linux)
Sources/KhmerAutocomplete/  the input method
    main.swift                IMKServer + candidate window + run loop
    InputController.swift     IMKInputController: keys, text before the caret, replacement, candidates, menu
    Runtime.swift             the engine (khdata.bin from the bundle) and the candidate panel, shared
    Settings.swift            preferences (Khmer / English state, apps where suggestions are off); never typed text
    SettingsWindow.swift      the preferences window
Tests/KhBridgeTests/        XCTest: the bridge against the repo's seed data
Resources/Info.plist        template (@BUNDLE_ID@, @VERSION@ are filled in by build.sh)
build.sh install.sh uninstall.sh check-plist.py     RELEASE.md (signing, notarization, installer, updates)
```

## Build and install (on a Mac)
```
mac/build.sh              # universal app in mac/build/ (ad-hoc signed; set SIGN_IDENTITY for a real signature)
mac/install.sh            # copies it to ~/Library/Input Methods, then add the source in System Settings
swift test --package-path mac -Xlinker -L$PWD/mac/lib    # bridge tests (after build.sh has made mac/lib)
python3 mac/check-plist.py                               # runs anywhere
```
Change `BUNDLE_ID` (default `org.khmerautocomplete.inputmethod.KhmerAutocomplete`) to your own reverse-DNS name before shipping.

## Behavior
Two states, like any keyboard: **Khmer** and **English**. Control + Option + Space (or the input menu, or the settings
window) toggles them. Stored modes from earlier builds (Khmer script, Latin, English → Khmer) all become Khmer.
- **English:** pure pass-through. The engine is not called, no candidates are shown.
- **Khmer** (engine mode `.auto`): **no key is remapped**. Letters, digits and punctuation land in the document as typed (the
  pending *run*) and the engine works out what they were: Khmerlish (`knhom` → `ខ្ញុំ`), an English word (`water` → `ទឹក`),
  keys of the Khmer (NiDA) layout typed on a US keyboard (`xjJuM` → `ខ្ញុំ`), or Khmer script from the OS keyboard (next-word
  suggestions, completions, typo fixes, as before). Space / Enter / Tab always go to the engine:
  - **Space** converts only a confident top candidate (exact Khmerlish, exact English, a NiDA reading made of known words);
    otherwise the space is an ordinary space and the text stays as typed. Prefix and typo matches are shown, not applied.
  - **Tab** takes the first candidate (a prefix or typo match too). **Enter** takes a real candidate of a pending run, otherwise
    it is a newline.
  - If nothing matches, the candidate window ends in two choices: the keys as Khmer (NiDA reading) or as typed (English).
  - Each candidate carries its own `replace_len_utf16` (Khmerlish and English replace the trailing letters, NiDA the whole run),
    so `hello,water` + Space gives `hello,ទឹក`.
  - NiDA is no longer live per key: the Khmer letters appear on Space / Tab or a click, not as each key goes down.
- Off in password fields (`IsSecureEventInputEnabled`), in apps listed in settings, and when `khdata.bin` is missing.

## What has NOT been verified (written on Linux, nothing here was compiled or run on macOS)
- The Swift sources pass a syntax check (tree-sitter-swift) only. No type check, no build, no run.
- InputMethodKit details to confirm on a Mac, in this order:
  1. that the input source appears after `install.sh` (the `Info.plist` keys, `tsInputModeScriptKey = smKhmer`, language `km`);
  2. that `IMKCandidates` shows without any marked text (this input method never sets marked text: it inserts text directly and
     predicts from the text before the caret). If the panel does not appear or is misplaced, the fallback is a small
     borderless `NSPanel` positioned from `client.attributes(forCharacterIndex:lineHeightRectangle:)`;
  3. `attributedSubstring(from:)` / `selectedRange()` in real apps (Safari, Chrome, TextEdit, Terminal, VS Code):
     some clients (Electron, terminals) return nothing or stale ranges, which makes the input method pass keys through;
  4. the `refreshSoon()` timing: after a key the app handles itself, candidates are recomputed on the next run loop turn;
  5. that Control + Option + Space does not clash with an existing shortcut;
  6. in the Khmer state, that ordinary letters really reach the document unchanged (no remapping any more) and that Space,
     Enter and Tab (always sent to the engine) are fast enough not to be felt. The `layoutMap` call is no longer used by the app.
- The menu item action receives a dictionary in some macOS versions; `selectMode` accepts either that or an `NSMenuItem`.
- No icon yet: add `tsInputMethodIconFileKey` / `tsInputModeMenuIconFileKey` with a template image.
