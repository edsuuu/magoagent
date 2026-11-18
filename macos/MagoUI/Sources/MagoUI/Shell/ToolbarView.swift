import SwiftUI

/// The SF Symbol that stands for each tool (the egui toolbar draws its own marks).
func toolSymbol(_ id: String) -> String {
    switch id {
    case "select": "cursorarrow"
    case "hand": "hand.raised"
    // the terminal's button is the chat's too: its dialog picks one
    case "terminal": "terminal"
    case "filetree": "folder"
    case "portal": "globe"
    case "sticky": "note.text"
    case "draw": "pencil.tip"
    case "text": "textformat"
    case "eraser": "eraser"
    case "rectangle": "rectangle"
    case "diamond": "diamond"
    case "ellipse": "circle"
    case "arrow": "arrow.up.right"
    case "line": "line.diagonal"
    case "frame": "rectangle.dashed"
    case "autoshape": "wand.and.stars"
    case "laser": "scope"
    case "fill": "paintbucket"
    case "lasso": "lasso"
    case "chat": "bubble.left"
    default: "questionmark"
    }
}

/// The toolbar: the main tools, a hair, and the drawing row's button; under it the drawing tools and, as a menu, the extra ones.
struct ToolbarView: View {
    let state: ShellState

    var body: some View {
        VStack(spacing: 8) {
            GlassRow {
                ForEach(state.tools.main) { tool in
                    GlassButton(symbol: toolSymbol(tool.id), tip: tool.label, on: state.tool == tool.id, badge: tool.badge) {
                        shellVerb("tool", ["id": tool.id])
                    }
                }
                GlassDivider()
                GlassButton(symbol: "square.on.circle", tip: "Ferramentas de desenho", on: state.drawing) { shellVerb("draw_toggle") }
            }
            if state.drawing {
                GlassRow {
                    ForEach(state.tools.drawing) { tool in
                        GlassButton(symbol: toolSymbol(tool.id), tip: tool.label, on: state.tool == tool.id, badge: tool.badge) {
                            shellVerb("tool", ["id": tool.id])
                        }
                    }
                    GlassDivider()
                    ExtraToolsMenu(state: state)
                }
                .transition(.move(edge: .top).combined(with: .scale(scale: 0.92)).combined(with: .opacity))
            }
        }
        .animation(.spring(response: 0.3, dampingFraction: 0.7), value: state.drawing)
    }
}

/// "Mais ferramentas": a native menu, the current extra tool wearing its mark on the button.
private struct ExtraToolsMenu: View {
    let state: ShellState

    var body: some View {
        let current = state.tools.extra.first { $0.id == state.tool }
        Menu {
            ForEach(state.tools.extra) { tool in
                Button {
                    shellVerb("tool", ["id": tool.id])
                } label: {
                    Label(tool.badge.isEmpty ? tool.label : "\(tool.label)    \(tool.badge)", systemImage: toolSymbol(tool.id))
                }
            }
        } label: {
            Image(systemName: current.map { toolSymbol($0.id) } ?? "ellipsis").font(.system(size: 15))
                .foregroundStyle(current == nil ? Color.primary.opacity(0.85) : Color.accent007aff)
                .frame(width: 30, height: 30)
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .frame(width: 30, height: 30)
        .help("Mais ferramentas")
    }
}

/// The hint under the bars, in the muted ink.
struct HintView: View {
    let text: String
    var body: some View {
        Text(text).font(.system(size: 13)).foregroundStyle(.secondary).multilineTextAlignment(.center).allowsHitTesting(false)
    }
}
