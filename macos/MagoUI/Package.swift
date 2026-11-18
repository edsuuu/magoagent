// swift-tools-version: 6.0
// STUB: replaced by the foundation (needs `resources: [.copy("Terminal/Resources")]` for the Hack font)
import PackageDescription

let package = Package(
    name: "MagoUI",
    platforms: [.macOS(.v14)],
    products: [.library(name: "MagoUI", targets: ["MagoUI"])],
    targets: [
        .target(name: "MagoUI", resources: [.copy("Terminal/Resources")]),
    ]
)
