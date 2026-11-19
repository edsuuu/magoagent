import AppKit
import CoreText

/// The grid's fonts at one size: Hack (the egui terminal's), Menlo when it can't load (same advance, 0.602 em).
struct TerminalFonts {
    let regular: CTFont
    let bold: CTFont
    let italic: CTFont
    let boldItalic: CTFont
    /// Hack has no bold: it is drawn twice a little apart, as the egui terminal does.
    let syntheticBold: Bool
    /// One cell, in points: the width is the font's advance, the height the egui terminal's 17 at 14 pt.
    let cell: CGSize
    let ascent: CGFloat
    let descent: CGFloat

    /// Hack comes from Rust at start (`mago_ui_font`, egui's own copy).
    init(size: CGFloat) {
        var base = CTFontCreateWithName("Hack" as CFString, size, nil)
        if (CTFontCopyFamilyName(base) as String) != "Hack" {
            base = CTFontCreateWithName("Menlo" as CFString, size, nil)
        }
        func with(_ traits: CTFontSymbolicTraits) -> CTFont? {
            CTFontCreateCopyWithSymbolicTraits(base, size, nil, traits, traits)
        }
        let boldFont = with(.traitBold)
        regular = base
        bold = boldFont ?? base
        syntheticBold = boldFont == nil
        italic = with(.traitItalic) ?? base
        boldItalic = with([.traitBold, .traitItalic]) ?? base
        // the cell is the Rust side's: `font * 1233 / 2048` by `font * 17 / 14` (src/terminal/mod.rs CELL)
        cell = CGSize(width: size * 1233 / 2048, height: size * 17 / 14)
        ascent = CTFontGetAscent(base)
        descent = CTFontGetDescent(base)
    }

    func font(bold wantsBold: Bool, italic wantsItalic: Bool) -> CTFont {
        switch (wantsBold, wantsItalic) {
        case (true, true): boldItalic
        case (true, false): bold
        case (false, true): italic
        case (false, false): regular
        }
    }
}

/// The filled parts of a block element (U+2580–U+259F) as fractions of the cell [x0, y0, x1, y1], and the
/// shade for the three shades: they fill their cell exactly, which a font's glyph does not
/// (the same table as src/terminal/glyphs.rs `block_parts`).
func terminalBlockParts(_ scalar: UInt32) -> (parts: [[CGFloat]], alpha: CGFloat)? {
    func eighth(_ n: UInt32) -> CGFloat { CGFloat(n) / 8 }
    let ul: [CGFloat] = [0, 0, 0.5, 0.5], ur: [CGFloat] = [0.5, 0, 1, 0.5]
    let ll: [CGFloat] = [0, 0.5, 0.5, 1], lr: [CGFloat] = [0.5, 0.5, 1, 1]
    switch scalar {
    case 0x2580: return ([[0, 0, 1, 0.5]], 1)
    case 0x2581...0x2588: return ([[0, 1 - eighth(scalar - 0x2580), 1, 1]], 1)
    case 0x2589...0x258F: return ([[0, 0, eighth(0x2590 - scalar), 1]], 1)
    case 0x2590: return ([[0.5, 0, 1, 1]], 1)
    case 0x2591: return ([[0, 0, 1, 1]], 0.25)
    case 0x2592: return ([[0, 0, 1, 1]], 0.5)
    case 0x2593: return ([[0, 0, 1, 1]], 0.75)
    case 0x2594: return ([[0, 0, 1, 0.125]], 1)
    case 0x2595: return ([[0.875, 0, 1, 1]], 1)
    case 0x2596: return ([ll], 1)
    case 0x2597: return ([lr], 1)
    case 0x2598: return ([ul], 1)
    case 0x2599: return ([ul, ll, lr], 1)
    case 0x259A: return ([ul, lr], 1)
    case 0x259B: return ([ul, ur, ll], 1)
    case 0x259C: return ([ul, ur, lr], 1)
    case 0x259D: return ([ur], 1)
    case 0x259E: return ([ur, ll], 1)
    case 0x259F: return ([ur, ll, lr], 1)
    default: return nil
    }
}

/// The arms of a light box-drawing line (─ │, the corners, tees and cross, U+2500–U+257F) from the cell's centre, and
/// whether its corner is round (╭ ╮ ╯ ╰): drawn as lines through the cell so they join, which the fallback font's glyph
/// (Hack's copy has none) does not, a ─ run came out dashed.
func terminalBoxArms(_ scalar: UInt32) -> (left: Bool, up: Bool, right: Bool, down: Bool, round: Bool)? {
    switch scalar {
    case 0x2500: (true, false, true, false, false)
    case 0x2502: (false, true, false, true, false)
    case 0x250C, 0x256D: (false, false, true, true, scalar == 0x256D)
    case 0x2510, 0x256E: (true, false, false, true, scalar == 0x256E)
    case 0x2514, 0x2570: (false, true, true, false, scalar == 0x2570)
    case 0x2518, 0x256F: (true, true, false, false, scalar == 0x256F)
    case 0x251C: (false, true, true, true, false)
    case 0x2524: (true, true, false, true, false)
    case 0x252C: (true, false, true, true, false)
    case 0x2534: (true, true, true, false, false)
    case 0x253C: (true, true, true, true, false)
    default: nil
    }
}
