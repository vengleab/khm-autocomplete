import Cocoa
import KhBridge

/// A small preferences window: the state (Khmer or English) and the apps where suggestions are off.
final class SettingsWindowController: NSWindowController, NSTextFieldDelegate {
    static let shared = SettingsWindowController()

    private let modePopUp = NSPopUpButton(frame: .zero, pullsDown: false)
    private let disabledField = NSTextField(string: "")

    private init() {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 440, height: 170),
                              styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Khmer Autocomplete Settings"
        window.isReleasedWhenClosed = false
        super.init(window: window)
        build(in: window)
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    private func build(in window: NSWindow) {
        let modeLabel = NSTextField(labelWithString: "Keyboard:")
        modePopUp.addItems(withTitles: ["Khmer (reads Khmerlish, English and Khmer-layout keys)", "English (off)"])
        modePopUp.target = self
        modePopUp.action = #selector(modeChanged(_:))

        let appsLabel = NSTextField(labelWithString: "No suggestions in these apps (bundle IDs, comma separated):")
        disabledField.placeholderString = "com.apple.Terminal, com.example.Editor"
        disabledField.delegate = self

        let note = NSTextField(wrappingLabelWithString:
            "Suggestions are always off in password fields. Switch between Khmer and English with Control + Option + Space. "
            + "Everything runs on this Mac; typed text is never stored or sent anywhere.")
        note.textColor = .secondaryLabelColor
        note.font = .systemFont(ofSize: NSFont.smallSystemFontSize)

        let top = NSStackView(views: [modeLabel, modePopUp])
        top.orientation = .horizontal
        let stack = NSStackView(views: [top, appsLabel, disabledField, note])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        stack.translatesAutoresizingMaskIntoConstraints = false
        window.contentView = NSView()
        window.contentView?.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor),
            stack.topAnchor.constraint(equalTo: window.contentView!.topAnchor),
            stack.bottomAnchor.constraint(lessThanOrEqualTo: window.contentView!.bottomAnchor),
            disabledField.widthAnchor.constraint(equalToConstant: 400),
            note.widthAnchor.constraint(equalToConstant: 400),
        ])
    }

    func show() {
        modePopUp.selectItem(at: Settings.shared.state == .khmer ? 0 : 1)
        disabledField.stringValue = Settings.shared.disabledApps.joined(separator: ", ")
        NSApp.activate(ignoringOtherApps: true)
        window?.center()
        showWindow(nil)
    }

    @objc private func modeChanged(_ sender: NSPopUpButton) {
        Settings.shared.state = sender.indexOfSelectedItem == 0 ? .khmer : .english
    }

    func controlTextDidChange(_ obj: Notification) {
        Settings.shared.disabledApps = disabledField.stringValue
            .split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
    }
}
