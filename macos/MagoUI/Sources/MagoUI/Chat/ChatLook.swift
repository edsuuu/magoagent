// The egui chat's colours and fonts, copied from it: the Look (chat/list/mod.rs `ink`), the UiColors it uses for the words
// (ui/mod.rs), the amber card (chat/steps.rs `AmberCard`) and the bubbles (chat/mod.rs `bubble_fill`).
import SwiftUI

struct ChatLook {
    let dark: Bool
    let bg, panel, surface, raised, band, selected, line, text, soft, muted, hover, strong, faint, edge: Color
    /// UiColors: the words of a message, their `code`, a failure, the grey blocks.
    let words, code, danger, quiet, surfaceHigh, surfaceLow: Color
    /// The bubbles: the person's in the accent, the agent's grey.
    let accent, agentBubble: Color
    /// The amber card (AmberCard): its ground, edge, title, note, the well inside and its edge.
    let amberFill, amberEdge, amberTitle, amberNote, well, wellEdge: Color

    static let green = Color(rgb: 0x3fb950)
    static let amber = Color(rgb: 0xf0a73a)
    static let red = Color(rgb: 0xf85149)
    static let blue = Color(rgb: 0x0a84ff)
    static let claude = Color(rgb: 0xd97757)
    static let ready = Color(rgb: 0x34c759)

    init(dark: Bool) {
        self.dark = dark
        if dark {
            (bg, panel, surface, raised, band, selected) = (Color(rgb: 0x121212), Color(rgb: 0x1c1c1e), Color(rgb: 0x2c2c2e), Color(rgb: 0x3a3a3c), Color(rgb: 0x1a1a1a), Color(rgb: 0x3a3a3c))
            (line, text, soft, muted, hover, strong, faint, edge) =
                (Color(rgb: 0x2c2c2e), Color(rgb: 0xdbdbdb), Color(rgb: 0xa8a8a8), Color(rgb: 0x8e8e8e), Color(rgb: 0x232325), Color(rgb: 0xf2f2f2), Color(rgb: 0x6e6e73), Color(rgb: 0x48484a))
            (words, code, danger, quiet, surfaceHigh, surfaceLow) = (Color(rgb: 0xe3e3e8), Color(rgb: 0xb1b9f9), Color(rgb: 0xf85149), Color(rgb: 0x7a7a7a), Color(rgb: 0x3c3c40), Color(rgb: 0x303033))
            (accent, agentBubble) = (Color(rgb: 0x0a84ff), Color(rgb: 0x3b3b3e))
            (amberFill, amberEdge, amberTitle, amberNote, well, wellEdge) =
                (Color(rgb: 0x2a2213), Color(rgb: 0x7a5a22), Color(rgb: 0xf6c46f), Color(rgb: 0xd8c7a6), Color(rgb: 0x17130b), Color(rgb: 0x3d2f14))
        } else {
            (bg, panel, surface, raised, band, selected) = (.white, Color(rgb: 0xf5f5f5), Color(rgb: 0xf0f0f0), Color(rgb: 0xe4e4e4), Color(rgb: 0xf5f5f5), Color(rgb: 0xe4e4e4))
            (line, text, soft, muted, hover, strong, faint, edge) =
                (Color.black.opacity(24 / 255), Color(rgb: 0x262626), Color(rgb: 0x5c5c5c), Color(rgb: 0x808080), Color(rgb: 0xebebeb), Color(rgb: 0x111111), Color(rgb: 0x9a9a9e), Color.black.opacity(36 / 255))
            (words, code, danger, quiet, surfaceHigh, surfaceLow) = (Color(rgb: 0x1b1b1f), Color(rgb: 0x5769f7), Color(rgb: 0xc92a2a), Color(rgb: 0x999999), Color(rgb: 0xf2f2f5), Color(rgb: 0xececee))
            (accent, agentBubble) = (Color(rgb: 0x007aff), Color(rgb: 0xe9e9eb))
            (amberFill, amberEdge, amberTitle, amberNote, well, wellEdge) =
                (Color(rgb: 0xfdf3e1), Color(rgb: 0xe6b866), Color(rgb: 0x8a5a00), Color(rgb: 0x6b5a3a), Color(rgb: 0xfffaf0), Color(rgb: 0xefd3a0))
        }
    }

    /// The header's square behind Claude's asterisk: the ground 12% of the way to its orange.
    var claudeSquare: Color { Color(rgb: dark ? 0x2a1e1a : 0xfaefeb) }

    /// A state's word in the list on the light look: its colour 40% of the way to black, to read on its pale ground.
    static func chipInkLight(_ state: String) -> Color {
        switch state {
        case "working": Color(rgb: 0x064f99)
        case "waiting": Color(rgb: 0x906423)
        default: Color(rgb: 0x266f30)
        }
    }

    /// White at `alpha` on the dark look, black on the light one (chat/mod.rs `tint`).
    func tint(_ alpha: Double) -> Color { (dark ? Color.white : Color.black).opacity(alpha / 255) }

    /// A state's colour in the list's chips (list/running.rs `chip`).
    static func chip(_ state: String) -> Color {
        switch state {
        case "working": blue
        case "waiting": amber
        default: green
        }
    }

    /// A state's dot in the header (header.rs `state_color`).
    func dot(_ state: String, ended: Bool = false) -> Color {
        switch state {
        case "working": Self.claude
        case "waiting": Self.amber
        default: ended ? faint : Self.ready
        }
    }
}

extension Font {
    /// Hack, the egui chat's monospace (registered from Rust at start).
    static func mono(_ size: CGFloat) -> Font { .custom("Hack", size: size) }
}

extension EnvironmentValues {
    /// The chat's look for the colour scheme in use.
    var chatLook: ChatLook { ChatLook(dark: colorScheme == .dark) }
}

/// Words in the UI font made bold as egui makes them (it has no bold weight): drawn again a pixel to the right.
struct Thick: View {
    let text: String
    let size: CGFloat
    let color: Color
    @Environment(\.displayScale) private var scale

    var body: some View {
        Text(text).font(.app(size)).foregroundStyle(color)
            .overlay(alignment: .topLeading) { Text(text).font(.app(size)).foregroundStyle(color).offset(x: 1 / scale).allowsHitTesting(false) }
    }
}
