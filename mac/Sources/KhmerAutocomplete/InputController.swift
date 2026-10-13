import Cocoa
import Carbon.HIToolbox
import InputMethodKit
import KhBridge

/// One controller per text field the user is typing in. It collects keys and the text before the caret,
/// asks the engine (through KhBridge), shows the candidates and inserts the chosen text. No language logic.
@objc(KhmerInputController)
final class KhmerInputController: IMKInputController {
    private var current: [Candidate] = []
    /// The word the user just accepted, as long as the caret has not moved and no key was typed since.
    private var justAccepted: String?
    private var acceptedCaret = NSNotFound

    // macOS virtual key codes
    private enum Code {
        static let tab: UInt16 = 48, space: UInt16 = 49, escape: UInt16 = 53
        static let enter: UInt16 = 36, keypadEnter: UInt16 = 76
        static let delete: UInt16 = 51, forwardDelete: UInt16 = 117
    }

    // MARK: IMKInputController

    override func recognizedEvents(_ sender: Any!) -> Int {
        Int(NSEvent.EventTypeMask.keyDown.rawValue)
    }

    override func activateServer(_ sender: Any!) {
        super.activateServer(sender)
        refresh()
    }

    override func deactivateServer(_ sender: Any!) {
        hideCandidates()
        super.deactivateServer(sender)
    }

    override func commitComposition(_ sender: Any!) {
        // nothing is held in a marked-text composition; text goes straight into the document
        hideCandidates()
    }

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event = event, event.type == .keyDown, let client = sender as? IMKTextInput else { return false }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

        // hot key to change mode: Control + Option + Space
        if event.keyCode == Code.space, flags.contains(.control), flags.contains(.option) {
            Settings.shared.cycleMode()
            justAccepted = nil
            refresh()
            return true
        }
        // shortcuts belong to the application
        if flags.contains(.command) || flags.contains(.control) { return false }
        guard suggestionsAllowed(for: client) else {
            hideCandidates()
            return false
        }

        let mode = Settings.shared.mode
        switch event.keyCode {
        case Code.tab where !flags.contains(.shift):
            return perform(.tab, client)
        case Code.enter, Code.keypadEnter:
            return perform(.enter, client)
        case Code.space:
            if mode == .khmer {  // a plain space in Khmer mode: let it through, it finishes the word
                forgetAccepted()
                refreshSoon()
                return false
            }
            return perform(.space, client)
        case Code.escape:
            hideCandidates()
            return false
        case Code.delete, Code.forwardDelete:
            forgetAccepted()
            refreshSoon()
            return false
        default:
            break
        }

        forgetAccepted()
        if mode == .khmer,
           let khmer = KhmerEngine.layoutMap(macKeyCode: event.keyCode, shift: flags.contains(.shift), altgr: flags.contains(.option)) {
            // Khmer mode types Khmer itself, from the physical key (Khmer NiDA layout)
            client.insertText(khmer, replacementRange: NSRange(location: NSNotFound, length: NSNotFound))
            refresh()
            return true
        }
        // Latin and English letters, arrows, etc.: the application inserts them; look again once it has
        refreshSoon()
        return false
    }

    // MARK: candidate panel

    override func candidates(_ sender: Any!) -> [Any]! {
        current.map { display($0) }
    }

    override func candidateSelected(_ candidateString: NSAttributedString!) {
        guard let text = candidateString?.string,
              let chosen = current.first(where: { display($0) == text }),
              let client = self.client() else { return }
        apply(chosen, to: client)
    }

    private func display(_ c: Candidate) -> String {
        if let r = c.roman, !r.isEmpty { return "\(c.word)  \(r)" }
        return c.word
    }

    private func showCandidates() {
        guard let panel = Runtime.shared.candidates else { return }
        if current.isEmpty {
            panel.hide()
        } else {
            panel.update()
            panel.show(kIMKLocateCandidatesBelowHint)
        }
    }

    private func hideCandidates() {
        current = []
        Runtime.shared.candidates?.hide()
    }

    // MARK: engine calls

    /// Suggestions are off in secure input fields (passwords), in apps the user disabled, and with no engine.
    private func suggestionsAllowed(for client: IMKTextInput) -> Bool {
        if IsSecureEventInputEnabled() { return false }
        if Runtime.shared.engine == nil { return false }
        if let id = client.bundleIdentifier(), Settings.shared.disabledApps.contains(id) { return false }
        return true
    }

    /// The text before the caret, or nil when there is a selection (then the field is left alone).
    private func textBeforeCaret(_ client: IMKTextInput) -> String? {
        let sel = client.selectedRange()
        guard sel.location != NSNotFound, sel.length == 0 else { return nil }
        let len = min(sel.location, Settings.shared.contextUnits)
        guard len > 0 else { return "" }
        return client.attributedSubstring(from: NSRange(location: sel.location - len, length: len))?.string
    }

    private func forgetAccepted() {
        justAccepted = nil
        acceptedCaret = NSNotFound
    }

    /// Space / Enter / Tab: ask the engine; either swallow the key and insert, or let it through.
    private func perform(_ key: SpecialKey, _ client: IMKTextInput) -> Bool {
        guard let engine = Runtime.shared.engine, let text = textBeforeCaret(client) else { return false }
        let accepted = caretMoved(client) ? nil : justAccepted
        let action = (try? engine.keyAction(textBeforeCaret: text, mode: Settings.shared.mode, key: key, justAccepted: accepted)) ?? .passThrough
        switch action {
        case .insert(let candidate):
            apply(candidate, to: client)
            return true
        case .passThrough:
            forgetAccepted()
            refreshSoon()
            return false
        }
    }

    /// Replace the pending text before the caret with the chosen word (no trailing space is added).
    private func apply(_ c: Candidate, to client: IMKTextInput) {
        let sel = client.selectedRange()
        guard sel.location != NSNotFound, sel.length == 0, sel.location >= c.replaceLenUtf16 else { return }
        let range = NSRange(location: sel.location - c.replaceLenUtf16, length: c.replaceLenUtf16)
        client.insertText(c.word, replacementRange: range)
        justAccepted = c.word
        acceptedCaret = client.selectedRange().location
        refresh()
    }

    private func caretMoved(_ client: IMKTextInput) -> Bool {
        acceptedCaret != NSNotFound && client.selectedRange().location != acceptedCaret
    }

    /// Recompute and show the suggestions for the current text.
    private func refresh() {
        guard let client = self.client(), suggestionsAllowed(for: client), let engine = Runtime.shared.engine,
              let text = textBeforeCaret(client) else {
            hideCandidates()
            return
        }
        if caretMoved(client) { forgetAccepted() }
        current = (try? engine.suggest(textBeforeCaret: text, mode: Settings.shared.mode, justAccepted: justAccepted)) ?? []
        showCandidates()
    }

    /// After a key the application handles itself, the text changes only once the key has been delivered.
    private func refreshSoon() {
        DispatchQueue.main.async { [weak self] in self?.refresh() }
    }

    // MARK: menu

    override func menu() -> NSMenu! {
        let menu = NSMenu()
        let items: [(String, InputMode)] = [("Khmer script", .khmer), ("Latin input (Khmerlish)", .latin), ("English → Khmer", .english)]
        for (title, mode) in items {
            let item = NSMenuItem(title: title, action: #selector(selectMode(_:)), keyEquivalent: "")
            item.target = self
            item.tag = Int(mode.rawValue)
            item.state = Settings.shared.mode == mode ? .on : .off
            menu.addItem(item)
        }
        menu.addItem(.separator())
        let settings = NSMenuItem(title: "Settings…", action: #selector(openSettings(_:)), keyEquivalent: "")
        settings.target = self
        menu.addItem(settings)
        return menu
    }

    @objc private func selectMode(_ sender: Any?) {
        // InputMethodKit passes a dictionary holding the menu item; be liberal about what arrives
        var item = sender as? NSMenuItem
        if item == nil, let d = sender as? NSDictionary { item = d["IMKCommandMenuItem"] as? NSMenuItem }
        guard let tag = item?.tag, let mode = InputMode(rawValue: Int32(tag)) else { return }
        Settings.shared.mode = mode
        forgetAccepted()
        refresh()
    }

    @objc private func openSettings(_ sender: Any?) {
        SettingsWindowController.shared.show()
    }
}
