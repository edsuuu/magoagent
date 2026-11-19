import SwiftUI

// What Rust publishes on the topic "terminal:<id>" (src/app/terminal/native.rs and src/terminal/snapshot.rs).
// Colors are 0xRRGGBB; a run's background is -1 for the theme's.

struct TerminalState: Decodable {
    var id: String
    var name: String
    var folder: String
    var icon: String
    var tint: UInt32?
    var maestro: Bool
    var tag: String?
    var state: String?
    var dot: UInt32?
    var status: String
    var note: String?
    var font: Double
    var look: TerminalLook
    var blurred: Bool
    var locked: Bool
    var focused: Bool
    var elevated: Bool
    var copyOnSelect: Bool
    var grid: TerminalGrid?
    var find: TerminalFind?
    var entries: [TerminalEntry]
    var composer: String?

    enum CodingKeys: String, CodingKey {
        case id, name, folder, icon, tint, maestro, tag, state, dot, status, note, font, look, blurred, locked, focused, elevated
        case copyOnSelect = "copy_on_select"
        case grid, find, entries, composer
    }
}

struct TerminalLook: Decodable, Equatable {
    var bg: UInt32
    var fg: UInt32
    var cursor: UInt32
    /// The 0x181818 of the app's own look: its outline is gray, any other theme's is its text's.
    var isDefault: Bool { bg == 0x181818 }
}

struct TerminalFind: Decodable {
    var query: String
    var summary: String
    var count: Int
    var invalid: Bool
}

struct TerminalEntry: Decodable, Identifiable {
    var id: String
    var label: String
    var check: Bool?
    var enabled: Bool
    var keys: String
    var sep: Bool
    var items: [TerminalEntry]
}

struct TerminalGrid: Decodable {
    var cols: Int
    var rows: [TerminalRow]
    var cursor: TerminalCursor?
    var offset: Int
    var history: Int
    var altScreen: Bool

    enum CodingKeys: String, CodingKey {
        case cols, rows, cursor, offset, history
        case altScreen = "alt_screen"
    }
}

struct TerminalRow: Decodable {
    var rev: UInt32
    var runs: [TerminalRun]
}

struct TerminalCursor: Decodable, Equatable {
    var row: Int
    var col: Int
    /// block, underline, beam or hollow.
    var shape: String
    var blink: Bool
}

/// A run of cells, an array in the JSON: [column, cells, text, fg, bg, style]. Style bits: 1 bold, 2 italic, 4 underline, 8 strikeout.
struct TerminalRun: Decodable {
    var col: Int
    var cells: Int
    var text: String
    var fg: UInt32
    var bg: Int64
    var style: UInt8

    init(from decoder: Decoder) throws {
        var array = try decoder.unkeyedContainer()
        col = try array.decode(Int.self)
        cells = try array.decode(Int.self)
        text = try array.decode(String.self)
        fg = try array.decode(UInt32.self)
        bg = try array.decode(Int64.self)
        style = try array.decode(UInt8.self)
    }
}

extension Color {
    init(rgb: UInt32, opacity: Double = 1) {
        self.init(.sRGB, red: Double((rgb >> 16) & 255) / 255, green: Double((rgb >> 8) & 255) / 255, blue: Double(rgb & 255) / 255, opacity: opacity)
    }
}

extension NSColor {
    convenience init(rgb: UInt32, alpha: CGFloat = 1) {
        self.init(srgbRed: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: alpha)
    }
}

extension TerminalLook {
    /// The strips over and under the grid: the background with the text color's 6% over it.
    var strip: UInt32 {
        func mix(_ shift: UInt32) -> UInt32 {
            let under = Double((bg >> shift) & 255), over = Double((fg >> shift) & 255)
            return UInt32((under * 0.94 + over * 0.06).rounded())
        }
        return mix(16) << 16 | mix(8) << 8 | mix(0)
    }
}
