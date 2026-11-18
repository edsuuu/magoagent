import AppKit

/// Keys by the names Rust reads (egui's `Key::from_name`), the ones `key_bytes` (src/terminal/keys.rs) turns into
/// what a terminal sends. Printable text never goes by here: it arrives as text (`insertText`).
enum TerminalKeys {
    private static let special: [UInt16: String] = [
        36: "Enter", 76: "Enter", 48: "Tab", 51: "Backspace", 117: "Delete", 53: "Escape",
        123: "ArrowLeft", 124: "ArrowRight", 125: "ArrowDown", 126: "ArrowUp",
        115: "Home", 119: "End", 116: "PageUp", 121: "PageDown", 114: "Insert",
        122: "F1", 120: "F2", 99: "F3", 118: "F4", 96: "F5", 97: "F6", 98: "F7", 100: "F8", 101: "F9", 109: "F10", 103: "F11", 111: "F12",
    ]

    /// The name of a key that is not text: arrows, Enter, Tab, Backspace, function keys…
    static func special(_ event: NSEvent) -> String? { special[event.keyCode] }

    /// The name of the key under Ctrl (A–Z, Space, 2, 6, brackets, backslash, minus), by what it types without modifiers.
    static func control(_ event: NSEvent) -> String? {
        guard let character = event.charactersIgnoringModifiers?.lowercased().first else { return nil }
        switch character {
        case "a"..."z": return String(character).uppercased()
        case " ": return "Space"
        case "2": return "Num2"
        case "6": return "Num6"
        case "[": return "OpenBracket"
        case "]": return "CloseBracket"
        case "\\": return "Backslash"
        case "-": return "Minus"
        default: return nil
        }
    }

    static func modifiers(_ flags: NSEvent.ModifierFlags) -> [String: Bool] {
        ["shift": flags.contains(.shift), "alt": flags.contains(.option), "ctrl": flags.contains(.control), "cmd": flags.contains(.command)]
    }
}
