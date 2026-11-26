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

/// The egui tree's palette (`filetree/ui/mod.rs::Palette`), by the card's theme.
struct FTPalette {
    let bg, border, fg, muted, hover, strong, ring, edge, accent: NSColor

    init(dark: Bool) {
        func rgba(_ hex: UInt32, _ alpha: CGFloat = 1) -> NSColor {
            NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255, blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
        }
        if dark {
            (bg, border, fg) = (rgba(0x2c2c2e), rgba(0x545458, 0.5), rgba(0xf5f5f7))
            (muted, hover, strong) = (rgba(0xebebf5, 0.6), rgba(0xffffff, 15.0 / 255), rgba(0xffffff, 36.0 / 255))
            (ring, edge, accent) = (rgba(0x0a84ff, 0.6), rgba(0x404043), rgba(0x0a84ff))
        } else {
            (bg, border, fg) = (rgba(0xffffff), rgba(0x000000, 31.0 / 255), rgba(0x1d1d1f))
            (muted, hover, strong) = (rgba(0x3c3c43, 0.6), rgba(0x000000, 13.0 / 255), rgba(0x000000, 46.0 / 255))
            (ring, edge, accent) = (rgba(0x007aff, 0.5), rgba(0xe0e0e0), rgba(0x007aff))
        }
    }
}

extension Color {
    init(_ color: NSColor) { self.init(nsColor: color) }
}

/// lucide's drawings, at their 24 grid, as the egui tree paints them (its `outline` strokes one point wide whatever the size).
@MainActor
enum FTGlyphs {
    static let paths: [String: String] = [
        "chevron-left": #"<path d="m15 18-6-6 6-6"/>"#,
        "chevron-right": #"<path d="m9 18 6-6-6-6"/>"#,
        "chevron-down": #"<path d="m6 9 6 6 6-6"/>"#,
        "list": #"<path d="M3 12h.01"/><path d="M3 18h.01"/><path d="M3 6h.01"/><path d="M8 12h13"/><path d="M8 18h13"/><path d="M8 6h13"/>"#,
        "chevrons-up-down": #"<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>"#,
        "git-branch": #"<line x1="6" x2="6" y1="3" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>"#,
        "panel-right": #"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M15 3v18"/>"#,
        "git-compare": #"<circle cx="18" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><path d="M13 6h3a2 2 0 0 1 2 2v7"/><path d="M11 18H8a2 2 0 0 1-2-2V9"/>"#,
        "search": #"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#,
        "server": #"<rect width="20" height="8" x="2" y="2" rx="2"/><rect width="20" height="8" x="2" y="14" rx="2"/><path d="M6 6h.01"/><path d="M6 18h.01"/>"#,
        "x": #"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#,
        "save": #"<path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"/><path d="M7 3v4a1 1 0 0 0 1 1h7"/>"#,
        "open": #"<path d="M21 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h6"/><path d="m21 3-9 9"/><path d="M15 3h6v6"/>"#,
        "settings": #"<path d="M20 7h-9"/><path d="M14 17H5"/><circle cx="17" cy="17" r="3"/><circle cx="7" cy="7" r="3"/>"#,
    ]
    private static var cache: [String: NSImage] = [:]

    /// `name` in `color`, `stroke` points wide when drawn `size` points across.
    static func image(_ name: String, color: NSColor, size: CGFloat, stroke: CGFloat) -> NSImage? {
        let rgb = color.usingColorSpace(.sRGB) ?? color
        let hex = String(format: "#%02X%02X%02X", Int(rgb.redComponent * 255), Int(rgb.greenComponent * 255), Int(rgb.blueComponent * 255))
        let key = "\(name)|\(hex)|\(rgb.alphaComponent)|\(size)|\(stroke)"
        if let image = cache[key] { return image }
        guard let body = paths[name] else { return nil }
        let svg = """
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="\(hex)" \
            stroke-opacity="\(rgb.alphaComponent)" stroke-width="\(stroke * 24 / size)" stroke-linecap="round" stroke-linejoin="round">\(body)</svg>
            """
        let image = NSImage(data: Data(svg.utf8))
        image?.size = NSSize(width: size, height: size)
        cache[key] = image
        return image
    }
}

/// One lucide drawing in a box of `size`.
struct FTGlyph: View {
    let name: String
    var size: CGFloat = 13
    let color: NSColor
    var stroke: CGFloat = 1

    var body: some View {
        if let image = FTGlyphs.image(name, color: color, size: size, stroke: stroke) {
            Image(nsImage: image).resizable().interpolation(.high).frame(width: size, height: size)
        } else {
            Color.clear.frame(width: size, height: size)
        }
    }
}

/// The app's font has one weight: egui's medium names are the regular struck twice, a hair apart.
struct FTMedium: View {
    let text: String
    var size: CGFloat = 13
    var color: Color

    var body: some View {
        Text(text).font(.app(size)).foregroundStyle(color)
            .overlay(Text(text).font(.app(size)).foregroundStyle(color).offset(x: 0.45))
            .lineLimit(1).truncationMode(.tail)
    }
}

/// A one-point line in the card's border color.
struct FTRule: View {
    let color: NSColor
    var body: some View { Rectangle().fill(Color(color)).frame(height: 1) }
}

/// The egui tree's scrollbar (`filetree/ui/draw.rs`), only while the rows run past the box: a 6 wide thumb 7 in from the
/// right edge and 3 from either end, at least 20 tall, `strong` (`muted` under the pointer or dragged), and the rows 13
/// narrower to leave it room.
/// ponytail: macOS 15 (scroll geometry and position); on 14 the system's overlay scroller stays.
struct FTScroller: ViewModifier {
    let pal: FTPalette

    func body(content: Content) -> some View {
        if #available(macOS 15, *) { content.modifier(Thumb(pal: pal)) } else { content }
    }

    @available(macOS 15, *)
    private struct Thumb: ViewModifier {
        let pal: FTPalette
        @State private var geometry: ScrollGeometry?
        @State private var position = ScrollPosition()
        @State private var hover = false
        @State private var from: CGFloat?

        private var overflows: Bool { geometry.map { $0.contentSize.height > $0.containerSize.height } ?? false }

        func body(content: Content) -> some View {
            content
                .scrollIndicators(.never)
                .contentMargins(.trailing, overflows ? 13 : 0, for: .scrollContent)
                .scrollPosition($position)
                .onScrollGeometryChange(for: ScrollGeometry.self, of: { $0 }) { _, new in geometry = new }
                .overlay(alignment: .topTrailing) { thumb }
        }

        @ViewBuilder private var thumb: some View {
            if let geometry, overflows {
                let (shown, total) = (geometry.containerSize.height, geometry.contentSize.height)
                let track = shown - 6
                let height = min(max(shown / total * track, 20), track)
                let span = max(total - shown, 1)
                let offset = min(max(geometry.contentOffset.y, 0), span)

                RoundedRectangle(cornerRadius: 3)
                    .fill(Color(hover || from != nil ? pal.muted : pal.strong))
                    .frame(width: 6, height: height)
                    .contentShape(Rectangle())
                    .onHover { hover = $0 }
                    .gesture(
                        DragGesture(minimumDistance: 0)
                            .onChanged { drag in
                                let start = from ?? offset

                                from = start
                                position.scrollTo(y: min(max(start + drag.translation.height / max(track - height, 1) * span, 0), span))
                            }
                            .onEnded { _ in from = nil }
                    )
                    .padding(.top, 3 + offset / span * (track - height))
                    .padding(.trailing, 7)
            }
        }
    }
}
