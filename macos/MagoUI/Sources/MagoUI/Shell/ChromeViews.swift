import SwiftUI

/// A round glass button of the window's top row (`titlebar.rs`): 32 across, the disc under the pointer 30, the icon at 1.05.
private struct TopButton: View {
    let icon: MagoIcon
    var scale: CGFloat = 1.05
    let on: Bool
    let tip: String
    let action: () -> Void

    var body: some View {
        GlassButton(icon: icon, tip: tip, on: on, scale: scale, action: action).frame(width: 32, height: 32).glass()
    }
}

/// The amber count of the chats waiting on the person, 3 in from the button's right and 4 down (`top_buttons`).
private struct ChatBadge: View {
    let count: Int
    var body: some View {
        Text("\(min(count, 99))").font(.app(10)).foregroundStyle(Color(white: 0x1d / 255))
            .frame(minWidth: 8).padding(.horizontal, 4).frame(minWidth: 16, minHeight: 16)
            .background(Capsule().fill(Color.amber))
            .overlay(Capsule().stroke(Color.themed(dark: 0x1a1a1a, light: 0xffffff), lineWidth: 1.5).padding(-0.75))
            .alignmentGuide(.top) { $0[VerticalAlignment.center] - 4 }
            .alignmentGuide(.trailing) { $0[HorizontalAlignment.center] + 3 }
            .allowsHitTesting(false)
    }
}

/// The canvas's top-left buttons: ☰ folds the sidebar (Ajustes over the chats), the chats' button with the badge of the ones
/// waiting, and with the sidebar folded the space in use by name.
struct TitleButtons: View {
    let state: ShellState

    var body: some View {
        let chats = state.screen == "chats"
        HStack(spacing: 6.5) {
            if chats {
                TopButton(icon: .gear, on: state.settingsOpen, tip: "Ajustes") { shellVerb("settings", ["open": !state.settingsOpen]) }
            } else {
                // ☰'s three bars read bigger than the other marks: a size down
                TopButton(icon: .menu, scale: 0.85, on: false, tip: state.sidebar.hidden ? "Abrir barra lateral" : "Fechar barra lateral") { shellVerb("sidebar") }
            }
            TopButton(icon: .composer, on: chats, tip: chats ? "Voltar ao canvas" : "Abrir o chat") {
                shellVerb("screen", ["state": chats ? "canvas" : "chats"])
            }
            .overlay(alignment: .topTrailing) { if state.chats.waiting > 0 { ChatBadge(count: state.chats.waiting) } }
            if state.sidebar.hidden && !chats, let space = state.space { SpacePill(space: space) }
        }
    }
}

/// With the sidebar folded: the space in use by name, a click opens the sidebar.
private struct SpacePill: View {
    let space: SpaceRef
    @State private var hovering = false

    var body: some View {
        Button { shellVerb("sidebar", ["state": "show"]) } label: {
            HStack(spacing: 6) {
                IconView(icon: .open, scale: 0.75).foregroundStyle(Color(rgba: space.color)).frame(width: 12)
                Text(space.name).font(.app(14)).foregroundStyle(Color.glassInk.opacity(0.9)).lineLimit(1)
            }
            .padding(.horizontal, 11).frame(height: 32)
            .background(Capsule().fill(Color.glassInk.opacity(hovering ? 0.06 : 0)))
            .glass()
            .contentShape(Capsule())
        }
        .buttonStyle(.plain).help("Abrir barra lateral").onHover { hovering = $0 }
    }
}

/// The panels on the window's right (`panes/mod.rs`): Alterações with its count of changed files, and Nota rápida.
struct PaneButtons: View {
    let pane: PaneState

    var body: some View {
        HStack(spacing: 6.5) {
            ForEach(pane.buttons) { button in
                let on = pane.open == button.id
                GlassButton(icon: button.id == "changes" ? .diff : .tool("sticky"), tip: button.name, on: on, scale: 1.05) {
                    appAction("panel", ["name": button.id, "open": !on])
                }
                .background(Circle().fill(Color.glassInk.opacity(on ? 0.1 : 0)).frame(width: 30, height: 30))
                .frame(width: 32, height: 32).glass()
                .overlay(alignment: .topTrailing) {
                    if button.count > 0 {
                        Text(button.count > 99 ? "99+" : "\(button.count)").font(.app(9.5)).foregroundStyle(.white)
                            .padding(.horizontal, 3.5).frame(minWidth: 14, minHeight: 14)
                            .background(Capsule().fill(Color.glassAccent))
                            .alignmentGuide(.top) { $0[VerticalAlignment.center] - 5 }
                            .alignmentGuide(.trailing) { $0[HorizontalAlignment.center] + 4 }
                            .allowsHitTesting(false)
                    }
                }
            }
        }
    }
}

/// The bottom left pill: Desfazer and Refazer, each off while there is nothing to undo or redo.
struct HistoryPill: View {
    let state: ShellState

    var body: some View {
        GlassRow(height: 38) {
            GlassButton(icon: .undo, tip: tip("Desfazer", "undo"), size: 28) { shellVerb("command", ["name": "undo"]) }.disabled(!state.canUndo)
            GlassButton(icon: .redo, tip: tip("Refazer", "redo"), size: 28) { shellVerb("command", ["name": "redo"]) }.disabled(!state.canRedo)
        }
    }

    private func tip(_ text: String, _ id: String) -> String {
        state.shortcuts.first { $0.id == id }.map { "\(text) — \($0.text)" } ?? text
    }
}

/// The bottom right pill: Ver tudo, the zoom (−, the percentage, +) and the minimap's toggle.
struct ZoomPill: View {
    let state: ShellState
    @State private var scrub: (x: CGFloat, zoom: Int)?
    @State private var hovering = false

    var body: some View {
        GlassRow(height: 38) {
            GlassButton(icon: .fit, tip: tip("Ver tudo", "fitAll"), size: 28) { shellVerb("zoom", ["op": "fit"]) }
            GlassDivider()
            GlassButton(icon: .minus, tip: tip("Diminuir zoom", "zoomOut"), size: 28) { shellVerb("zoom", ["op": "out"]) }
            // dragging sideways scrubs the zoom, 4 points to a 5% step; a click puts it back at 100%
            Text("\(state.zoom)%").font(.system(size: 13, design: .monospaced)).foregroundStyle(Color.glassInk.opacity(0.85)).frame(width: 48, height: 20)
                .background(RoundedRectangle(cornerRadius: 10).fill(Color.glassInk.opacity(hovering ? 0.1 : 0)))
                .contentShape(Rectangle()).help(tip("Redefinir zoom", "zoomReset"))
                .onHover { hovering = $0 }
                .onTapGesture { shellVerb("zoom", ["op": "reset"]) }
                .gesture(DragGesture(minimumDistance: 3).onChanged { drag in
                    let start = scrub ?? (drag.startLocation.x, state.zoom)
                    scrub = start
                    shellVerb("zoom", ["op": "set", "value": Double(start.zoom) / 100 + Double((drag.location.x - start.x) / 4) * 0.05])
                }.onEnded { _ in scrub = nil })
            GlassButton(icon: .plus, tip: tip("Aumentar zoom", "zoomIn"), size: 28) { shellVerb("zoom", ["op": "in"]) }
            GlassDivider()
            GlassButton(icon: .map, tip: "Minimapa", on: state.minimap, size: 28) { shellVerb("command", ["name": "minimap"]) }
        }
    }

    private func tip(_ text: String, _ id: String) -> String {
        state.shortcuts.first { $0.id == id }.map { "\(text) — \($0.text)" } ?? text
    }
}

/// The floors, left of the zoom (`floors/pill.rs`): the layers and the floor on screen by name ("Andares" while the overview is
/// open); a click opens or closes the overview, a right click lists the floors and Novo Andar.
struct FloorsPill: View {
    let state: ShellState
    @Binding var sheet: SpaceSheet?
    @State private var hovering = false

    var body: some View {
        let has = state.floors.count > 1
        let open = state.floorsOverview
        Button { shellVerb("command", ["name": "floors"]) } label: {
            HStack(spacing: 4) {
                IconView(icon: .layers(open), scale: 0.85).frame(width: 13)
                if has { Text(open ? "Andares" : state.floor).font(.app(11)).foregroundStyle(Color.glassInk).lineLimit(1).frame(maxWidth: 120, alignment: .leading) }
            }
            .foregroundStyle(open ? Color.glassAccent : Color.glassInk.opacity(0.85))
            .padding(.horizontal, has ? 8 : 0).frame(minWidth: 38).frame(height: 33)
            .background(Capsule().fill(Color.glassInk.opacity(hovering ? 0.1 : 0)))
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .padding(.horizontal, 1).frame(height: 38).glass()
        .help("Andares")
        .accessibilityLabel("Andares: \(state.floor)")
        .contextMenu {
            // the ground first, then the floors up
            ForEach(Array(state.floors.reversed().enumerated()), id: \.element.id) { index, floor in
                if index == 1 { Divider() }
                Button {
                    appAction("floors", ["action": "switch", "name": floor.id.isEmpty ? "Térreo" : floor.id])
                } label: {
                    if floor.current { Label(floor.name, systemImage: "checkmark") } else { Text(floor.name) }
                }
            }
            Divider()
            Button("Novo Andar") { sheet = .newFloor }
        }
    }
}

/// The message of the moment on glass, at the foot of the canvas (`status_toast`): as wide as its text, which wraps at 480;
/// one line is a pill, more lines round off less.
struct ToastView: View {
    let toast: Toast

    var body: some View {
        ViewThatFits(in: .horizontal) {
            pill(Text(toast.text).fixedSize(), radius: nil)
            pill(Text(toast.text).fixedSize(horizontal: false, vertical: true).frame(width: 480, alignment: .leading), radius: 16)
        }
        .frame(width: 480 + 32)
        .allowsHitTesting(false)
    }

    private func pill(_ text: some View, radius: CGFloat?) -> some View {
        text.font(.app(13.5)).foregroundStyle(Color.glassInk).padding(.horizontal, 16).padding(.vertical, 10).glass(radius: radius, strong: true)
    }
}
