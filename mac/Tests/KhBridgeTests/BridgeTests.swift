import XCTest
@testable import KhBridge

/// Runs against the seed data in the repository (data/). Needs libkhcore_capi.a (see mac/build.sh).
final class BridgeTests: XCTestCase {
    private func repoFile(_ rel: String) -> String {
        // mac/Tests/KhBridgeTests/BridgeTests.swift -> repo root is three levels up
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent(rel).path
    }

    private func engine() throws -> KhmerEngine {
        try KhmerEngine(dataPath: repoFile("data/data.json"), lexiconPath: repoFile("data/lexicon.json"), englishPath: repoFile("data/english.json"))
    }

    func testAbiVersion() {
        XCTAssertEqual(KhmerEngine.abiVersion, 1)
    }

    func testKhmerCompletion() throws {
        let c = try engine().suggest(textBeforeCaret: "ខ្ញុំ ស្រ", mode: .khmer)
        XCTAssertEqual(c.first?.word, "ស្រលាញ់")
        XCTAssertEqual(c.first?.replace, "ស្រ")
        XCTAssertEqual(c.first?.replaceLenUtf16, 3)  // base + coeng + subscript: three UTF-16 units
    }

    func testSpaceConvertsLatinWord() throws {
        let e = try engine()
        guard case .insert(let c) = try e.keyAction(textBeforeCaret: "knyom", mode: .latin, key: .space) else { return XCTFail("expected insert") }
        XCTAssertEqual(c.word, "ខ្ញុំ")
        XCTAssertEqual(c.replaceLenUtf16, 5)
        XCTAssertEqual(try e.keyAction(textBeforeCaret: "", mode: .latin, key: .space), .passThrough)
    }

    func testEnglishMode() throws {
        let c = try engine().suggest(textBeforeCaret: "water", mode: .english)
        XCTAssertEqual(c.first?.word, "ទឹក")
        XCTAssertEqual(c.first?.roman, "water")
    }

    func testLayout() {
        XCTAssertEqual(KhmerEngine.layoutMap(macKeyCode: 40, shift: false, altgr: false), "ក")  // K
        XCTAssertEqual(KhmerEngine.layoutMap(macKeyCode: 40, shift: true, altgr: false), "គ")
        XCTAssertNil(KhmerEngine.layoutMap(macKeyCode: 49, shift: false, altgr: false))         // Space is not ours
    }

    func testBadPathThrows() {
        XCTAssertThrowsError(try KhmerEngine(compiledDataPath: "/no/such/file"))
    }
}
