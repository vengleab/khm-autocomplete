import Foundation
import KhBridge

/// User settings. Only preferences are stored here: typed text is never saved or logged.
final class Settings {
    static let shared = Settings()
    private let defaults = UserDefaults.standard

    /// The current input mode (also changed from the menu and the hot key).
    var mode: InputMode {
        get { InputMode(rawValue: Int32(defaults.integer(forKey: "mode"))) ?? .khmer }
        set { defaults.set(Int(newValue.rawValue), forKey: "mode") }
    }

    /// Bundle identifiers of apps where suggestions are off (comma separated in the settings window).
    var disabledApps: [String] {
        get { defaults.stringArray(forKey: "disabledApps") ?? [] }
        set { defaults.set(newValue, forKey: "disabledApps") }
    }

    /// How many UTF-16 units before the caret are sent to the engine. Enough for the last few words.
    let contextUnits = 96

    func cycleMode() {
        switch mode {
        case .khmer: mode = .latin
        case .latin: mode = .english
        case .english: mode = .khmer
        }
    }
}
