// Thin Swift wrapper over the khcore C API (capi/include/khcore.h).
// No language logic lives here: strings go in, JSON comes back, and it is decoded into plain values.
import CKhCore
import Foundation

public enum InputMode: Int32 {
    case khmer = 0    // type Khmer script (Khmer NiDA layout), get completions and next words
    case latin = 1    // type Khmer in Latin letters (Khmerlish)
    case english = 2  // type English words, get Khmer suggestions
}

public enum SpecialKey: Int32 {
    case space = 0
    case enter = 1
    case tab = 2
}

/// One suggestion, as the engine reports it.
public struct Candidate: Decodable, Equatable {
    public let word: String
    public let kind: String
    /// the text the word replaces (normalized form)
    public let replace: String
    /// how many UTF-16 units before the caret to replace: an `NSRange` length, measured in the text we sent
    public let replaceLenUtf16: Int
    /// insert after the word before the caret instead of replacing it
    public let append: Bool
    public let prob: Double?
    /// Latin spelling (Latin mode) or matched English gloss (English mode)
    public let roman: String?
    public let why: String?
    /// key presses this suggestion saves
    public let gain: Int

    enum CodingKeys: String, CodingKey {
        case word, kind, replace, append, prob, roman, why, gain
        case replaceLenUtf16 = "replace_len_utf16"
    }
}

public enum KeyAction: Equatable {
    /// not ours: let the app handle the key
    case passThrough
    /// swallow the key, replace `replaceLenUtf16` units before the caret with `word`, add nothing else
    case insert(Candidate)
}

public struct EngineError: Error, CustomStringConvertible {
    public let description: String
}

public final class KhmerEngine {
    private let handle: OpaquePointer

    private static func lastError(_ fallback: String) -> EngineError {
        if let p = kh_last_error() { return EngineError(description: String(cString: p)) }
        return EngineError(description: fallback)
    }

    /// Open compiled data made by `khbuild` (the fast path used by the app).
    public init(compiledDataPath: String) throws {
        guard let h = kh_engine_open_compiled(compiledDataPath) else { throw KhmerEngine.lastError("cannot open \(compiledDataPath)") }
        handle = h
    }

    /// Open from the interchange files (slow start; for tests and tools).
    public init(dataPath: String, lexiconPath: String, englishPath: String? = nil) throws {
        let h: OpaquePointer?
        if let english = englishPath {
            h = kh_engine_open_with_english(dataPath, lexiconPath, english)
        } else {
            h = kh_engine_open(dataPath, lexiconPath)
        }
        guard let opened = h else { throw KhmerEngine.lastError("cannot open engine") }
        handle = opened
    }

    deinit { kh_engine_free(handle) }

    private func take(_ p: UnsafeMutablePointer<CChar>?) throws -> Data {
        guard let p = p else { throw KhmerEngine.lastError("engine call failed") }
        defer { kh_string_free(p) }
        return Data(bytes: p, count: Int(strlen(p)))
    }

    /// Candidates for the text before the caret, best first (at most 8).
    /// `justAccepted`: the word the user just accepted, until the next key press or caret move.
    public func suggest(textBeforeCaret: String, mode: InputMode, justAccepted: String? = nil) throws -> [Candidate] {
        let data = try take(kh_suggest(handle, textBeforeCaret, mode.rawValue, justAccepted))
        return try JSONDecoder().decode([Candidate].self, from: data)
    }

    /// What to do with Space / Enter / Tab. `textBeforeCaret` excludes the key being pressed.
    public func keyAction(textBeforeCaret: String, mode: InputMode, key: SpecialKey, justAccepted: String? = nil) throws -> KeyAction {
        struct Raw: Decodable {
            let action: String
            let candidate: Candidate?
        }
        let data = try take(kh_key_action(handle, textBeforeCaret, mode.rawValue, key.rawValue, justAccepted))
        let raw = try JSONDecoder().decode(Raw.self, from: data)
        if raw.action == "insert", let c = raw.candidate { return .insert(c) }
        return .passThrough
    }

    /// What a physical key types in Khmer mode (Khmer NiDA layout), or nil when the key is not part of it.
    /// `keyCode` is `NSEvent.keyCode`; `altgr` is the Option key.
    public static func layoutMap(macKeyCode: UInt16, shift: Bool, altgr: Bool) -> String? {
        guard let p = kh_layout_map(KH_KEYCODE_MAC, UInt32(macKeyCode), shift ? 1 : 0, altgr ? 1 : 0) else { return nil }
        defer { kh_string_free(p) }
        return String(cString: p)
    }

    /// The C interface version this build was linked against.
    public static var abiVersion: UInt32 { kh_abi_version() }
}
