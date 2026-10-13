import Cocoa
import InputMethodKit
import KhBridge

/// Process-wide state shared by every input controller: the engine and the candidate window.
final class Runtime {
    static let shared = Runtime()

    /// nil when the data file is missing or unreadable: the input method then lets every key through.
    private(set) var engine: KhmerEngine?
    var candidates: IMKCandidates?

    private init() {
        guard let url = Bundle.main.url(forResource: "khdata", withExtension: "bin") else {
            NSLog("KhmerAutocomplete: khdata.bin is missing from the app bundle; suggestions are off")
            return
        }
        do {
            engine = try KhmerEngine(compiledDataPath: url.path)
        } catch {
            // never log typed text; this is only a file problem
            NSLog("KhmerAutocomplete: cannot load khdata.bin: \(error)")
        }
    }
}
