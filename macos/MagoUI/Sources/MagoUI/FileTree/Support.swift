import AppKit
import SwiftUI

extension NSColor {
    /// `#RRGGBB` or `#RRGGBBAA`.
    convenience init(hex: String) {
        var digits = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        if digits.count == 6 { digits += "FF" }
        let value = UInt64(digits, radix: 16) ?? 0xFF00FFFF
        self.init(
            srgbRed: CGFloat((value >> 24) & 0xFF) / 255,
            green: CGFloat((value >> 16) & 0xFF) / 255,
            blue: CGFloat((value >> 8) & 0xFF) / 255,
            alpha: CGFloat(value & 0xFF) / 255)
    }
}

extension Color {
    init(hex: String) { self.init(nsColor: NSColor(hex: hex)) }
}

/// What the file tree says to Rust: `ui.filetree.<verb>` with the tree's id.
@MainActor
enum FT {
    static func send(_ verb: String, _ id: String, _ args: [String: Any] = [:]) {
        var merged = args
        merged["id"] = id
        MagoStore.shared.send("ui.filetree.\(verb)", merged)
    }

    /// The same, waiting for Rust's answer (an error comes back as `ok == false` with its words).
    @discardableResult
    static func act(_ verb: String, _ id: String, _ args: [String: Any] = [:]) async -> MagoReply {
        var merged = args
        merged["id"] = id
        return await MagoStore.shared.act("ui.filetree.\(verb)", merged)
    }

    static func state(_ id: String) -> FTState? { MagoStore.shared.decode(FTState.self, topic: "filetree:\(id)") }

    /// The editor's colors never change: read once they are published.
    private static var cachedTheme: FTTheme?
    static var theme: FTTheme? {
        if cachedTheme == nil { cachedTheme = MagoStore.shared.decode(FTTheme.self, topic: "filetree:theme") }
        return cachedTheme
    }

    private static var cachedIcons: [String: FTIconPair]?
    private static var images: [String: NSImage] = [:]

    /// A JetBrains icon (an embedded SVG Rust sends once, in the dark or the light palette).
    static func icon(_ key: String, dark: Bool) -> NSImage? {
        let name = "\(key)/\(dark)"
        if let image = images[name] { return image }
        if cachedIcons == nil { cachedIcons = MagoStore.shared.decode([String: FTIconPair].self, topic: "filetree:icons") }
        guard let pair = cachedIcons?[key], let image = NSImage(data: Data((dark ? pair.dark : pair.light).utf8)) else { return nil }
        images[name] = image
        return image
    }
}

/// The path from `root` (or the path itself when it is outside).
func ftRelative(_ path: String, to root: String) -> String {
    let base = root.hasSuffix("/") ? root : root + "/"
    return path.hasPrefix(base) ? String(path.dropFirst(base.count)) : path
}

/// The name and the extension of a file name: the extension keeps its dot, and stays when renaming.
func ftSplitName(_ name: String) -> (String, String) {
    guard let cut = name.lastIndex(of: "."), cut != name.startIndex else { return (name, "") }
    return (String(name[..<cut]), String(name[cut...]))
}

/// The word git's letter stands for.
func ftStatusWord(_ letter: String) -> String {
    switch letter {
    case "M": "modificado"
    case "A": "adicionado"
    case "D": "excluído"
    case "R": "renomeado"
    case "U": "novo, não rastreado"
    default: "em conflito"
    }
}
