import SwiftUI

/// The window's whole content: the sidebar on the left, the canvas (or the chats) filling the rest with the bars floating over it.
struct ShellRoot: View {
    private let model = ShellModel.shared
    @State private var sheet: SpaceSheet?
    @State private var toast: Toast?

    var body: some View {
        Group {
            if let state = model.state {
                ShellContent(state: state, sheet: $sheet, toast: toast)
                    .preferredColorScheme(state.theme == "system" ? nil : state.dark ? .dark : .light)
                    .onChange(of: state, initial: true) { _, next in ShellController.shared.apply(next) }
                    .onChange(of: state.toast) { _, next in show(next) }
            } else {
                // the core has not said yet
                Color.clear
            }
        }
        .onChange(of: model.notice) { _, next in show(next) }
        .ignoresSafeArea()
    }

    private func show(_ next: Toast?) {
        guard let next else { return }
        withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) { toast = next }
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(next.seconds))
            if toast?.id == next.id { withAnimation(.easeOut(duration: 0.2)) { toast = nil } }
        }
    }
}

struct ShellContent: View {
    let state: ShellState
    @Binding var sheet: SpaceSheet?
    let toast: Toast?

    private var chats: Bool { state.screen == "chats" }
    private var sidebarShown: Bool { !state.sidebar.hidden && !chats }

    var body: some View {
        ZStack {
            HStack(spacing: 0) {
                if sidebarShown {
                    SidebarView(state: state, sheet: $sheet).transition(.move(edge: .leading))
                }
                canvasColumn
            }
            .animation(.spring(response: 0.38, dampingFraction: 0.9), value: sidebarShown)
            if state.settingsOpen { SettingsCover() }
            DialogsView(state: state, sheet: $sheet)
        }
    }

    /// What fills the window beside the sidebar, and the chrome that floats over it.
    private var canvasColumn: some View {
        ZStack(alignment: .top) {
            content.ignoresSafeArea()
            if !chats {
                VStack(spacing: 8) {
                    ToolbarView(state: state)
                    if let properties = state.properties {
                        PropertiesRow(props: properties, state: state)
                            .transition(.move(edge: .top).combined(with: .scale(scale: 0.9)).combined(with: .opacity))
                    }
                    if let hint = state.hint { HintView(text: hint) }
                }
                .padding(.top, 12).padding(.horizontal, 12)
                .animation(.spring(response: 0.3, dampingFraction: 0.7), value: state.properties != nil)
            }
            HStack(alignment: .top) {
                TitleButtons(state: state)
                Spacer()
                if !chats { PaneButtons(pane: state.pane) }
            }
            .padding(.top, 16).padding(.leading, sidebarShown ? 12 : 84).padding(.trailing, 12)
            if !chats {
                VStack {
                    Spacer()
                    HStack(alignment: .bottom, spacing: 6.5) {
                        HistoryPill(state: state)
                        Spacer()
                        FloorsPill(state: state, sheet: $sheet)
                        ZoomPill(state: state)
                    }
                    .padding(10)
                }
            }
            if let toast {
                VStack { Spacer(); ToastView(toast: toast).padding(.bottom, 24) }.transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// The area that is on: the canvas or the chats, else a placeholder saying which one has not registered.
    @ViewBuilder private var content: some View {
        if chats {
            if let view = MagoViews.chats { view() } else { Placeholder(text: "Chats") }
        } else if let view = MagoViews.canvas {
            view()
        } else {
            Placeholder(text: "Canvas")
        }
    }
}

private struct Placeholder: View {
    let text: String
    var body: some View {
        ZStack {
            Color(nsColor: .windowBackgroundColor)
            VStack(spacing: 6) {
                Image(systemName: "rectangle.dashed").font(.app(28)).foregroundStyle(.tertiary)
                Text("\(text) ainda não registrado").font(.app(13)).foregroundStyle(.secondary)
            }
        }
    }
}

/// Ajustes: the settings area over a dimmed window.
private struct SettingsCover: View {
    var body: some View {
        ZStack {
            Color.black.opacity(0.28).ignoresSafeArea().onTapGesture { shellVerb("settings", ["open": false]) }
            ZStack(alignment: .topTrailing) {
                Group {
                    if let view = MagoViews.settings { view() } else { Placeholder(text: "Ajustes") }
                }
                .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                Button { shellVerb("settings", ["open": false]) } label: { Image(systemName: "xmark.circle.fill").font(.app(18)).foregroundStyle(.secondary) }
                    .buttonStyle(.plain).keyboardShortcut(.cancelAction).padding(10).help("Fechar Ajustes")
            }
            .frame(maxWidth: 980, maxHeight: 700)
            .padding(40)
            .shadow(color: .black.opacity(0.3), radius: 24, y: 8)
        }
        .transition(.opacity)
    }
}
