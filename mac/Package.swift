// swift-tools-version:5.7
import PackageDescription

// Build with mac/build.sh, which first builds the Rust static library into mac/lib and passes
// `-Xlinker -L<mac/lib>` so that `link "khcore_capi"` in the module map finds libkhcore_capi.a.
let package = Package(
    name: "KhmerAutocomplete",
    platforms: [.macOS(.v11)],
    products: [
        .executable(name: "KhmerAutocomplete", targets: ["KhmerAutocomplete"]),
    ],
    targets: [
        // the C header of khcore-capi (symlink to capi/include/khcore.h)
        .systemLibrary(name: "CKhCore", path: "Sources/CKhCore"),
        // thin Swift wrapper over the C API: no AppKit, no language logic
        .target(name: "KhBridge", dependencies: ["CKhCore"], path: "Sources/KhBridge"),
        // the input method: InputMethodKit controller, candidate window, settings
        .executableTarget(name: "KhmerAutocomplete", dependencies: ["KhBridge"], path: "Sources/KhmerAutocomplete"),
        .testTarget(name: "KhBridgeTests", dependencies: ["KhBridge"], path: "Tests/KhBridgeTests"),
    ]
)
