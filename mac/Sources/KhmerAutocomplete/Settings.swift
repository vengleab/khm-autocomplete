import Foundation
import KhBridge

/// User settings. Only preferences are stored here: typed text is never saved or logged.
final class Settings {
    static let shared = Settings()
    private let defaults = UserDefaults.standard

    /// The two states of the keyboard. In `.khmer` the engine reads what was typed (Khmerlish, English, keys of the
    /// Khmer layout, Khmer script); `.english` is pure pass-through, the engine is not called.
    enum State: Int {
        case english = 0
        case khmer = 1
    }

    /// The engine mode behind the Khmer state.
    let engineMode: InputMode = .auto

    private init() {
        // earlier versions stored one of three modes (0, 1, 2) under "mode"; all of them are the Khmer state now
        defaults.removeObject(forKey: "mode")
    }

    /// The current state (also changed from the menu and the hot key). The default is Khmer.
    var state: State {
        get { (defaults.object(forKey: "state") as? Int).flatMap(State.init(rawValue:)) ?? .khmer }
        set { defaults.set(newValue.rawValue, forKey: "state") }
    }

    /// Bundle identifiers of apps where suggestions are off (comma separated in the settings window).
    var disabledApps: [String] {
        get { defaults.stringArray(forKey: "disabledApps") ?? [] }
        set { defaults.set(newValue, forKey: "disabledApps") }
    }

    /// How many UTF-16 units before the caret are sent to the engine. Enough for the last few words.
    let contextUnits = 96

    func toggleState() {
        state = state == .khmer ? .english : .khmer
    }
}
