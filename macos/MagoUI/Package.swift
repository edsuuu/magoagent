// swift-tools-version: 6.0
// A interface SwiftUI do MagoAgent no macOS: uma biblioteca estática que o build.rs do app compila e liga.
// O contrato com o Rust está em docs/swiftui.md e no cabeçalho Sources/MagoBridgeC/include/mago_ui.h.
import PackageDescription

let package = Package(
    name: "MagoUI",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "MagoUI", type: .static, targets: ["MagoUI"])
    ],
    targets: [
        // só o cabeçalho C do contrato (a struct MagoCallbacks que o Rust passa)
        .target(name: "MagoBridgeC"),
        .target(name: "MagoUI", dependencies: ["MagoBridgeC"]),
    ]
)
