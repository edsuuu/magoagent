import SwiftUI

/// A tool's icon on its button: the terminal's button is the chat's too (its dialog picks one), so it wears the chat's mark.
func toolIcon(_ id: String) -> MagoIcon {
    .tool(id == "terminal" ? "chat" : id)
}

/// The toolbar: the main tools, a hair, and the drawing row's button; under it the drawing tools and "Mais ferramentas".
struct ToolbarView: View {
    let state: ShellState

    var body: some View {
        VStack(spacing: 8) {
            GlassRow {
                ForEach(state.tools.main) { tool in
                    GlassButton(icon: toolIcon(tool.id), tip: tool.label, on: state.tool == tool.id, badge: tool.badge) {
                        shellVerb("tool", ["id": tool.id])
                    }
                }
                GlassDivider()
                GlassButton(icon: .shapes, tip: "Ferramentas de desenho", on: state.drawing) { shellVerb("draw_toggle") }
            }
            if state.drawing {
                GlassRow {
                    ForEach(state.tools.drawing) { tool in
                        GlassButton(icon: toolIcon(tool.id), tip: tool.label, on: state.tool == tool.id, badge: tool.badge) {
                            shellVerb("tool", ["id": tool.id])
                        }
                    }
                    GlassDivider()
                    ExtraToolsButton(state: state)
                }
                .transition(.move(edge: .top).combined(with: .scale(scale: 0.92)).combined(with: .opacity))
            }
        }
        .animation(.spring(response: 0.3, dampingFraction: 0.7), value: state.drawing)
    }
}

/// "Mais ferramentas": ⋮, or the extra tool in use; its menu lists them, the one in use in the accent.
private struct ExtraToolsButton: View {
    let state: ShellState
    @State private var open = false

    var body: some View {
        let current = state.tools.extra.first { $0.id == state.tool }
        GlassButton(icon: current.map { toolIcon($0.id) } ?? .more, tip: "Mais ferramentas", on: current != nil || open) { open.toggle() }
            .popover(isPresented: $open, arrowEdge: .bottom) {
                ForEach(state.tools.extra) { tool in
                    MenuRow(icon: toolIcon(tool.id), label: tool.label, current: tool.id == state.tool) {
                        shellVerb("tool", ["id": tool.id])
                        open = false
                    }
                }
                .menuBox(width: 230)
            }
    }
}

/// The hint under the bars, in the muted ink.
struct HintView: View {
    let text: String
    var body: some View {
        Text(text).font(.app(13)).foregroundStyle(MenuLook.muted).multilineTextAlignment(.center).allowsHitTesting(false)
    }
}
