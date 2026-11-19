import AppKit
import SwiftUI

/// The open panel on the window's right (app/panes/mod.rs `panes_ui`): under the title bar's row, as wide as the topic says,
/// its name and × on top, the body scrolling, a bar pinned to the bottom (Nota rápida's, or Alterações' comment), the left
/// edge pulled to widen it. Nothing while closed; the canvas gives it room (ShellRoot pads the canvas by `inset`).
struct PaneColumn: View {
    private let model = PanesModel.shared

    var body: some View {
        if let state = model.state, let open = state.open {
            PaneBox(state: state, open: open)
                .frame(width: CGFloat(state.width))
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .trailing)
                .transition(.move(edge: .trailing))
        }
    }
}

/// The panel's top (egui's `PANE_TOP`) and its heading's height.
private let paneTop: CGFloat = 60
private let paneHead: CGFloat = 46

private struct PaneBox: View {
    let state: PanesState
    let open: String

    private var look: PaneLook { state.look }
    private var line: Color { Color(rgba: look.ink).opacity(0.12) }

    var body: some View {
        VStack(spacing: 0) {
            head
            Rectangle().fill(line).frame(height: 1)
            if open == "note" {
                // 12 from the edge as egui's: the text view adds its own 5
                QuickNoteBody(note: state.note, look: look)
                    .padding(.leading, 7).padding(.trailing, 12).padding(.top, 12)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            } else if let changes = state.changes {
                ScrollView(.vertical) {
                    VStack(alignment: .leading, spacing: 10) {
                        ChangesBody(changes: changes, look: look, rows: state.listRows)
                    }
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            } else {
                Spacer(minLength: 0)
            }
            if let bar = bottomBar {
                Rectangle().fill(line).frame(height: 1)
                bar
            }
        }
        .foregroundStyle(Color(rgba: look.text))
        .background(Color(rgba: look.bg))
        .overlay(alignment: .leading) { PaneEdge(look: look, width: state.width) }
        .padding(.top, paneTop)
        .background(alignment: .top) { Color(rgba: look.bg).frame(height: paneTop) }
        .contentShape(Rectangle())
    }

    private var head: some View {
        HStack {
            Text(open == "note" ? "Nota rápida" : "Alterações").font(.app(14))
            Spacer()
            PaneClose(look: look)
        }
        .padding(.leading, 14).padding(.trailing, 8)
        .frame(height: paneHead)
    }

    /// The bar pinned to the bottom: Nota rápida's always, Alterações' while lines are picked.
    private var bottomBar: AnyView? {
        if open == "note" { return AnyView(QuickNoteBar(note: state.note, look: look)) }
        if let changes = state.changes, let comment = changes.comment { return AnyView(CommentBar(title: comment, look: look)) }
        return nil
    }
}

/// The × in the heading: a square lit under the pointer.
private struct PaneClose: View {
    let look: PaneLook
    @State private var hovering = false

    var body: some View {
        Button { appAction("panel", ["open": false]) } label: {
            Canvas { context, size in
                let middle = CGPoint(x: size.width / 2, y: size.height / 2)
                var cross = Path()
                cross.move(to: CGPoint(x: middle.x - 4.5, y: middle.y - 4.5))
                cross.addLine(to: CGPoint(x: middle.x + 4.5, y: middle.y + 4.5))
                cross.move(to: CGPoint(x: middle.x - 4.5, y: middle.y + 4.5))
                cross.addLine(to: CGPoint(x: middle.x + 4.5, y: middle.y - 4.5))
                context.stroke(cross, with: .color(Color(rgba: look.ink).opacity(0.85)), lineWidth: 1.25)
            }
            .frame(width: 28, height: 28)
            .background(Color(rgba: look.ink).opacity(hovering ? 0.1 : 0), in: RoundedRectangle(cornerRadius: 6))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help("Fechar")
        .accessibilityLabel("Fechar")
    }
}

/// The left edge: pulled, it widens or narrows the panel (the MCP's `panel` width, kept in the prefs); double-clicked, back
/// to the default.
private struct PaneEdge: View {
    let look: PaneLook
    let width: Double
    @State private var hovering = false
    @State private var start: Double?

    var body: some View {
        Rectangle().fill(Color(rgba: look.ink).opacity(0.12)).frame(width: 1)
            .overlay {
                Rectangle().fill(Color(rgba: look.accent)).frame(width: 2).opacity(hovering || start != nil ? 1 : 0)
            }
            .frame(width: 8)
            .contentShape(Rectangle())
            .offset(x: -4)
            .onHover { inside in
                hovering = inside
                if inside { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() }
            }
            .gesture(DragGesture(minimumDistance: 1, coordinateSpace: .global).onChanged { drag in
                let from = start ?? width
                start = from
                appAction("panel", ["width": max(1, from - Double(drag.translation.width))])
            }.onEnded { _ in start = nil })
            .onTapGesture(count: 2) { appAction("panel", ["width": 0]) }
    }
}

/// egui's button in the panels: the raised field color, rounded 6, 12 by 6 of room, the interface text; faded when off.
struct PaneButtonStyle: ButtonStyle {
    let look: PaneLook

    func makeBody(configuration: Configuration) -> some View {
        PaneButtonLabel(configuration: configuration, look: look)
    }
}

private struct PaneButtonLabel: View {
    let configuration: ButtonStyleConfiguration
    let look: PaneLook
    @Environment(\.isEnabled) private var enabled
    @State private var hovering = false

    var body: some View {
        configuration.label
            .font(.app(14))
            .foregroundStyle(Color(rgba: enabled ? look.text : look.muted))
            .padding(.horizontal, 12)
            .frame(minHeight: 28)
            .background(Color(rgba: hovering && enabled ? look.raised : look.field), in: RoundedRectangle(cornerRadius: 6))
            .opacity(configuration.isPressed ? 0.8 : 1)
            .onHover { hovering = $0 }
    }
}

/// egui's text field in the panels: the field color, rounded 6, no border.
extension View {
    func paneField(_ look: PaneLook, height: CGFloat? = nil) -> some View {
        self
            .textFieldStyle(.plain)
            .font(.app(14))
            .padding(.horizontal, 12)
            .frame(minHeight: height ?? 28)
            .background(Color(rgba: look.field), in: RoundedRectangle(cornerRadius: 6))
    }
}
