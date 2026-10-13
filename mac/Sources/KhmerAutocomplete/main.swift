import Cocoa
import InputMethodKit

// An input method is an ordinary app that registers an IMKServer under the connection name from Info.plist.
// macOS starts it when the user selects the input source and talks to it through that connection.
guard let connectionName = Bundle.main.object(forInfoDictionaryKey: "InputMethodConnectionName") as? String else {
    NSLog("KhmerAutocomplete: InputMethodConnectionName is missing from Info.plist")
    exit(1)
}

let server = IMKServer(name: connectionName, bundleIdentifier: Bundle.main.bundleIdentifier)
// one candidate window for the whole process; the active controller supplies the candidates
Runtime.shared.candidates = IMKCandidates(server: server, panelType: kIMKSingleRowSteppingCandidatePanel)

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
app.run()
