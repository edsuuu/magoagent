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

    /// Registered once for the process.
    private static let registered: Bool = {
        let bundle = Bundle.module
        guard let url = bundle.url(forResource: "Hack-Regular", withExtension: "ttf", subdirectory: "Resources")
            ?? bundle.url(forResource: "Hack-Regular", withExtension: "ttf")
        else { return false }
        return CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
    }()

    init(size: CGFloat) {
        _ = Self.registered
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
