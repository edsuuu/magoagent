// swift-tools-version:6.0
// STUB: replaced by the foundation
import PackageDescription

let package = Package(
    name: "MagoUI",
    platforms: [.macOS(.v14)],
    products: [.library(name: "MagoUI", type: .static, targets: ["MagoUI"])],
    targets: [.target(name: "MagoUI")]
)
