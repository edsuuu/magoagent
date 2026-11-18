import SwiftUI

/// The canvas's top-left buttons: ☰ folds the sidebar (Ajustes over the chats), the chats' button with the badge of the ones waiting,
/// and with the sidebar folded the space in use by name.
struct TitleButtons: View {
    let state: ShellState

    var body: some View {
        let chats = state.screen == "chats"
        HStack(spacing: 6.5) {
            round(chats ? "gearshape" : "line.3.horizontal", on: chats && state.settingsOpen,
                  tip: chats ? "Ajustes" : state.sidebar.hidden ? "Abrir barra lateral" : "Fechar barra lateral") {
                if chats { shellVerb("settings", ["open": !state.settingsOpen]) } else { shellVerb("sidebar") }
            }
            round("bubble.left.and.bubble.right", on: chats, tip: chats ? "Voltar ao canvas" : "Abrir o chat") {
                shellVerb("screen", ["state": chats ? "canvas" : "chats"])
            }
            .overlay(alignment: .topTrailing) {
                if state.chats.waiting > 0 {
                    Text("\(min(state.chats.waiting, 99))").font(.system(size: 10, weight: .semibold)).foregroundStyle(Color(white: 0.11))
                        .padding(.horizontal, 4).frame(minWidth: 16, minHeight: 16)
                        .background(Capsule().fill(Color.amber).overlay(Capsule().stroke(Color(nsColor: .windowBackgroundColor), lineWidth: 1.5)))
                        .offset(x: 6, y: -4)
                }
            }
            if state.sidebar.hidden && !chats, let space = state.space {
                Button { shellVerb("sidebar", ["state": "show"]) } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "folder.fill").font(.system(size: 12)).foregroundStyle(Color(rgba: space.color))
                        Text(space.name).font(.system(size: 14)).lineLimit(1)
                    }
                    .padding(.horizontal, 11).frame(height: 32).glass()
                }
                .buttonStyle(.plain).help("Abrir barra lateral")
            }
            if state.dev {
                Text("DEV").font(.system(size: 11, weight: .semibold)).foregroundStyle(Color(white: 0.12))
                    .padding(.horizontal, 8).frame(height: 18).background(Capsule().fill(Color.amber))
            }
        }
    }

    private func round(_ symbol: String, on: Bool, tip: String, action: @escaping () -> Void) -> some View {
        GlassButton(symbol: symbol, tip: tip, on: on, size: 32, action: action).glass()
    }
}

/// The panels on the window's right (Alterações): a round button each.
struct PaneButtons: View {
    let pane: PaneState

    var body: some View {
        HStack(spacing: 6.5) {
            ForEach(pane.buttons) { button in
                GlassButton(symbol: "plus.forwardslash.minus", tip: button.name, on: pane.open == button.id, size: 32) {
                    appAction("panel", ["name": button.id, "open": pane.open != button.id])
                }
                .glass()
            }
        }
    }
}

/// The bottom left pill: Desfazer and Refazer, each off while there is nothing to undo or redo.
struct HistoryPill: View {
    let state: ShellState

    var body: some View {
        GlassRow(height: 38) {
            GlassButton(symbol: "arrow.uturn.backward", tip: tip("Desfazer", "undo"), size: 28) { shellVerb("command", ["name": "undo"]) }.disabled(!state.canUndo)
            GlassButton(symbol: "arrow.uturn.forward", tip: tip("Refazer", "redo"), size: 28) { shellVerb("command", ["name": "redo"]) }.disabled(!state.canRedo)
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

    var body: some View {
        GlassRow(height: 38) {
            GlassButton(symbol: "arrow.up.left.and.down.right.magnifyingglass", tip: "Ver tudo", size: 28) { shellVerb("zoom", ["op": "fit"]) }
            GlassDivider()
            GlassButton(symbol: "minus", tip: "Diminuir zoom", size: 28) { shellVerb("zoom", ["op": "out"]) }
            // dragging sideways scrubs the zoom, 4 points to a 5% step; a click puts it back at 100%
            Text("\(state.zoom)%").font(.system(size: 13, design: .monospaced)).foregroundStyle(.primary.opacity(0.85)).frame(width: 48, height: 20)
                .contentShape(Rectangle()).help("Redefinir zoom")
                .onTapGesture { shellVerb("zoom", ["op": "reset"]) }
                .gesture(DragGesture(minimumDistance: 3).onChanged { drag in
                    let start = scrub ?? (drag.startLocation.x, state.zoom)
                    scrub = start
                    shellVerb("zoom", ["op": "set", "value": Double(start.zoom) / 100 + Double((drag.location.x - start.x) / 4) * 0.05])
                }.onEnded { _ in scrub = nil })
            GlassButton(symbol: "plus", tip: "Aumentar zoom", size: 28) { shellVerb("zoom", ["op": "in"]) }
            GlassDivider()
            GlassButton(symbol: "map", tip: "Minimapa", on: state.minimap, size: 28) { shellVerb("command", ["name": "minimap"]) }
        }
    }
}

/// The floors, left of the zoom: the one on screen by name, a menu of the others.
struct FloorsPill: View {
    let state: ShellState

    var body: some View {
        if state.floors.count > 1 || state.floorsOverview {
            Menu {
                ForEach(state.floors) { floor in
                    Button {
                        appAction("floors", ["action": "switch", "name": floor.id.isEmpty ? "Térreo" : floor.id])
                    } label: {
                        if floor.current { Label(floor.name, systemImage: "checkmark") } else { Text(floor.name) }
                    }
                }
                Divider()
                Button("Visão dos andares") { shellVerb("command", ["name": "floors"]) }
            } label: {
                Label(state.floor, systemImage: "square.3.layers.3d").font(.system(size: 13))
                    .padding(.horizontal, 12).frame(height: 38).glass()
            }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .help("Andares")
        }
    }
}

/// The message of the moment on glass, at the foot of the canvas.
struct ToastView: View {
    let toast: Toast

    var body: some View {
        Text(toast.text).font(.system(size: 13.5)).multilineTextAlignment(.center).frame(maxWidth: 480)
            .padding(.horizontal, 16).padding(.vertical, 10)
            .glass(radius: toast.text.count > 60 ? 16 : nil)
            .allowsHitTesting(false)
    }
}
