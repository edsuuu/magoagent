import SwiftUI

/// Where the grid's NSView is, to give it the keys back (after the search bar or the message box).
@MainActor
final class TerminalGridHandle {
    weak var view: TerminalGridView?

    func focus() {
        guard let view else { return }
        view.window?.makeFirstResponder(view)
    }
}

/// A terminal's card: the title strip, the grid, the folder strip, the search bar (⌘F) and, under the card, the
/// message box. Everything it shows is the topic "terminal:<id>"; what it does goes back as `ui.terminal.*` verbs
/// or the MCP's actions.
public struct TerminalView: View {
    let id: String

    public init(id: String) { self.id = id }

    @State private var handle = TerminalGridHandle()
    @State private var findOpen = false
    @State private var query = ""
    @State private var procsOpen = false
    @FocusState private var findFocused: Bool

    private static let radius: CGFloat = 8
    private static let headerHeight: CGFloat = 28
    private static let footerHeight: CGFloat = 22

    public var body: some View {
        let state = MagoStore.shared.decode(TerminalState.self, topic: "terminal:\(id)")
        let look = state?.look ?? TerminalLook(bg: 0x181818, fg: 0xd8d8d8, cursor: 0xaaaaaa)
        VStack(spacing: 0) {
            Color.clear.frame(height: Self.headerHeight)
            ZStack(alignment: .topTrailing) {
                Color(rgb: look.bg)
                if let state {
                    TerminalSurface(id: id, state: state, handle: handle, onFind: { openFind(state) })
                    if state.blurred { veil(look) }
                    if findOpen || state.find != nil { findBar(state, look) }
                }
            }
            if let state { footer(state, look) }
        }
        .background(Color(rgb: look.bg))
        .clipShape(RoundedRectangle(cornerRadius: Self.radius))
        .overlay(RoundedRectangle(cornerRadius: Self.radius).strokeBorder(look.isDefault ? Color(white: 0x3a / 255) : Color(rgb: look.fg, opacity: 0.2)))
        .overlay(alignment: .top) { if let state { header(state, look) } }
        .overlay(alignment: .bottom) {
            if let state, let draft = state.composer {
                TerminalComposer(id: id, name: state.name, remote: draft, onClose: { handle.focus() })
                    .padding(.horizontal, 12)
                    .alignmentGuide(.bottom) { $0[.top] - 14 }
            }
        }
        .onAppear { MagoStore.shared.send("ui.terminal.visible", ["id": id, "visible": true]) }
        .onDisappear { MagoStore.shared.send("ui.terminal.visible", ["id": id, "visible": false]) }
    }

    // MARK: title strip

    private func header(_ state: TerminalState, _ look: TerminalLook) -> some View {
        let ink = Color(rgb: look.fg)
        return HStack(spacing: 4.875) {
            TerminalMark(icon: state.icon, color: Color(rgb: state.tint ?? look.fg)).frame(width: 11)
            Text(state.name).font(.app(9.75)).foregroundStyle(ink).lineLimit(1)
            if state.maestro { TerminalCrown().frame(width: 13, height: 10).padding(.leading, -1.5) }
            if let tag = state.tag {
                Text(tag).font(.app(9.75)).foregroundStyle(ink.opacity(0.6)).lineLimit(1).padding(.leading, 4.875)
            }
            Spacer(minLength: 0)
            if let dot = state.dot {
                Circle().fill(Color(rgb: dot)).frame(width: 6.5, height: 6.5).padding(.trailing, state.elevated ? 34 : 0).help(Self.stateLabel(state.state))
            }
        }
        .padding(.horizontal, 8.125)
        .frame(height: Self.headerHeight)
        .background(Color(rgb: look.strip))
        .overlay(alignment: .bottom) { Rectangle().fill(Color(rgb: look.fg, opacity: 0.14)).frame(height: 1) }
        .overlay(alignment: .trailing) {
            if state.elevated {
                Button { MagoStore.shared.send("resize_window", ["target": id, "state": "shrink"]) } label: {
                    TerminalMark(icon: "expand", color: ink).frame(width: 22, height: 22)
                }
                .buttonStyle(.plain)
                .padding(.trailing, 7)
                .help("Voltar ao canvas")
            }
        }
    }

    private static func stateLabel(_ state: String?) -> String {
        switch state {
        case "working": "Trabalhando"
        case "active": "Ativo"
        case "waiting": "Esperando você"
        case "idle": "Parado"
        case "ended": "Encerrado"
        default: ""
        }
    }

    // MARK: veil

    /// Desfocar: the screen goes behind a veil of the terminal's color, with an eye to lift it.
    private func veil(_ look: TerminalLook) -> some View {
        ZStack {
            Color(rgb: look.bg)
            Button { MagoStore.shared.send("terminal_menu", ["name": id, "action": "blur", "value": "off"]) } label: {
                Image(systemName: "eye.slash").font(.system(size: 22)).foregroundStyle(Color(rgb: look.fg, opacity: 0.25)).frame(width: 44, height: 44)
            }
            .buttonStyle(.plain)
            .help("Remover Desfoque")
        }
    }

    // MARK: folder strip

    private func footer(_ state: TerminalState, _ look: TerminalLook) -> some View {
        let muted = Color(rgb: look.fg, opacity: 0.6)
        return HStack(spacing: 4.875) {
            Text(state.folder).font(.custom("Hack", size: 10)).foregroundStyle(muted).lineLimit(1).truncationMode(.head)
            Spacer(minLength: 0)
            if let note = state.note {
                Text(note).font(.custom("Hack", size: 10)).foregroundStyle(Color(rgb: look.fg)).lineLimit(1)
                    .onTapGesture { if state.status == "exited" { MagoStore.shared.send("terminal_restart", ["name": id]) } }
            }
            if state.status == "running" {
                Button { procsOpen.toggle() } label: {
                    Image(systemName: "cpu").font(.system(size: 10)).foregroundStyle(muted).frame(width: 18, height: 16)
                }
                .buttonStyle(.plain)
                .help("Processos")
                .popover(isPresented: $procsOpen, arrowEdge: .top) { TerminalProcesses(id: id) }
            }
        }
        .padding(.horizontal, 8.125)
        .frame(height: Self.footerHeight)
        .background(Color(rgb: look.strip))
        .overlay(alignment: .top) { Rectangle().fill(Color(rgb: look.fg, opacity: 0.14)).frame(height: 1) }
    }

    // MARK: search

    /// ⌘F: the bar opens with what is selected in it (not a line break).
    private func openFind(_ state: TerminalState) {
        findOpen = true
        findFocused = true
        Task {
            let reply = await MagoStore.shared.act("ui.terminal.copy", ["id": id])
            if !reply.text.isEmpty, !reply.text.contains("\n") { query = reply.text }
        }
    }

    private func closeFind() {
        findOpen = false
        query = ""
        MagoStore.shared.send("terminal_find", ["name": id, "action": "close"])
        handle.focus()
    }

    private func findBar(_ state: TerminalState, _ look: TerminalLook) -> some View {
        let ink = Color(rgb: look.fg)
        let summary = state.find?.summary ?? ""
        let any = (state.find?.count ?? 0) > 0
        return HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").font(.system(size: 11)).foregroundStyle(ink.opacity(0.6))
            TextField("Buscar", text: $query)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(ink)
                .frame(width: 150)
                .focused($findFocused)
                .onSubmit {
                    let backwards = NSEvent.modifierFlags.contains(.shift)
                    MagoStore.shared.send("terminal_find", ["name": id, "action": backwards ? "previous" : "next"])
                    findFocused = true
                }
                .onExitCommand { closeFind() }
                .onChange(of: query) { _, text in
                    guard !text.isEmpty else { return }
                    MagoStore.shared.send("terminal_find", ["name": id, "action": "find", "query": text])
                }
            Text(summary).font(.system(size: 12)).foregroundStyle(ink.opacity(0.6)).frame(width: 76, alignment: .trailing).lineLimit(1)
            Rectangle().fill(ink.opacity(0.2)).frame(width: 1, height: 14)
            stepButton("chevron.up", tip: "Ocorrência anterior", on: any, ink: ink) { MagoStore.shared.send("terminal_find", ["name": id, "action": "previous"]) }
            stepButton("chevron.down", tip: "Próxima ocorrência", on: any, ink: ink) { MagoStore.shared.send("terminal_find", ["name": id, "action": "next"]) }
            stepButton("xmark", tip: "Fechar a barra de busca", on: true, ink: ink) { closeFind() }
        }
        .padding(.horizontal, 10)
        .frame(height: 28)
        .background(.regularMaterial, in: Capsule())
        .overlay(Capsule().strokeBorder(ink.opacity(0.12), lineWidth: 0.5))
        .padding(.top, 10)
        .padding(.trailing, 8.125)
    }

    private func stepButton(_ symbol: String, tip: String, on: Bool, ink: Color, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 10, weight: .semibold)).foregroundStyle(ink.opacity(on ? 1 : 0.4)).frame(width: 18, height: 18)
        }
        .buttonStyle(.plain)
        .disabled(!on)
        .help(tip)
    }
}

/// The right-click menu's rows, in the app's order; a submenu nests.
struct EntryItems: View {
    let entries: [TerminalEntry]
    let state: TerminalState

    var body: some View {
        ForEach(Array(entries.enumerated()), id: \.offset) { _, entry in
            if entry.sep {
                Divider()
            } else if !entry.items.isEmpty {
                Menu(entry.label) { AnyView(EntryItems(entries: entry.items, state: state)) }
            } else {
                Button {
                    TerminalActions.pick(entry.id, state: state)
                } label: {
                    if entry.check == true { Label(entry.label, systemImage: "checkmark") } else { Text(entry.label) }
                }
                .disabled(!entry.enabled)
            }
        }
    }
}

/// The message box under the terminal: a pill that grows with the text; Enter sends it as a prompt, Shift+Enter breaks
/// the line, Esc closes. Its draft lives in Rust too (a prompt that could not be typed comes back to it).
struct TerminalComposer: View {
    let id: String
    let name: String
    /// The draft as Rust has it.
    let remote: String
    let onClose: () -> Void

    @State private var draft = ""
    @State private var lastSent = ""
    @State private var failure: String?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .bottom, spacing: 6) {
                TextField("Mensagem para \(name) · Esc para fechar", text: $draft, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1...10)
                    .padding(.vertical, 6)
                    .focused($focused)
                    .onKeyPress(.return, phases: .down) { press in
                        guard !press.modifiers.contains(.shift) else { return .ignored }
                        send()
                        return .handled
                    }
                    .onKeyPress(.escape) {
                        MagoStore.shared.send("terminal_menu", ["name": id, "action": "message_box", "value": "off"])
                        onClose()
                        return .handled
                    }
                Button(action: send) {
                    Image(systemName: "arrow.up").font(.system(size: 13, weight: .bold)).foregroundStyle(.white)
                        .frame(width: 32, height: 32)
                        .background(Circle().fill(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? Color(white: 0x55 / 255) : Color(rgb: 0x0a84ff)))
                }
                .buttonStyle(.plain)
                .help("Enviar")
            }
            .padding(.leading, 18)
            .padding(4)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 22))
            if let failure { Text(failure).font(.system(size: 11)).foregroundStyle(.red).padding(.leading, 18) }
        }
        .frame(maxWidth: 560)
        .onAppear {
            draft = remote
            lastSent = remote
            focused = true
        }
        // what Rust has that this did not send is new: a prompt that came back
        .onChange(of: remote) { _, text in
            if text != lastSent {
                draft = text
                lastSent = text
            }
        }
        .onChange(of: draft) { _, text in
            lastSent = text
            MagoStore.shared.send("ui.terminal.composer", ["id": id, "text": text])
        }
    }

    private func send() {
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        Task {
            let reply = await MagoStore.shared.act("ui.terminal.prompt", ["id": id, "text": text])
            if reply.ok {
                draft = ""
                failure = nil
            } else {
                failure = reply.text
            }
        }
    }
}
