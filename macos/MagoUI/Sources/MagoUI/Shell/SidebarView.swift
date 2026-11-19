import AppKit
import SwiftUI

/// What the sidebar's own sheets ask for (Swift keeps them: the names are typed here and the actions are the app's).
enum SpaceSheet: Identifiable, Equatable {
    case newSpace
    case removeSpace(id: String, name: String, running: Int)
    case newGroup
    case renameGroup(id: String, name: String)
    case removeGroup(id: String, name: String)

    var id: String {
        switch self {
        case .newSpace: "new-space"
        case .removeSpace(let id, _, _): "remove-space-\(id)"
        case .newGroup: "new-group"
        case .renameGroup(let id, _): "rename-group-\(id)"
        case .removeGroup(let id, _): "remove-group-\(id)"
        }
    }
}

/// The window's left column: Filtrar and +, Precisa de você, the agents of the space, the spaces (Fixados, loose, groups), and at the foot
/// Novo grupo and Ajustes.
struct SidebarView: View {
    let state: ShellState
    @Binding var sheet: SpaceSheet?
    @State private var filter = ""
    @State private var renaming: String?
    @State private var renamed = ""

    private var side: SidebarState { state.sidebar }

    private var visibleSpaces: [SpaceRow] {
        let wanted = filter.trimmingCharacters(in: .whitespaces).lowercased()
        return wanted.isEmpty ? side.spaces : side.spaces.filter { $0.name.lowercased().contains(wanted) }
    }

    var body: some View {
        VStack(spacing: 0) {
            // under the window's buttons, which sit over the sidebar's top
            HStack(spacing: 8) {
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass").font(.system(size: 12)).foregroundStyle(.secondary)
                    TextField("Filtrar", text: $filter).textFieldStyle(.plain).font(.system(size: 14))
                }
                .padding(.horizontal, 12).frame(height: 32)
                .background(Capsule().fill(Color.primary.opacity(0.06)))
                GlassButton(symbol: "plus", tip: "Novo espaço de trabalho") { sheet = .newSpace }
            }
            .padding(.horizontal, 10).padding(.top, 54)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    AttentionCard(state: state)
                    AgentsSection(state: state)
                    Heading("ESPAÇOS DE TRABALHO")
                    if visibleSpaces.isEmpty { Text("Nenhum espaço de trabalho corresponde à busca.").font(.system(size: 13)).foregroundStyle(.secondary).padding(8) }
                    let pinned = visibleSpaces.filter(\.pinned)
                    if !pinned.isEmpty {
                        Heading("FIXADOS")
                        ForEach(pinned) { row($0, indent: 0) }
                    }
                    ForEach(visibleSpaces.filter { !$0.pinned && $0.group.isEmpty }) { row($0, indent: 0) }
                    ForEach(side.groups) { group in
                        let inside = visibleSpaces.filter { !$0.pinned && $0.group == group.id }
                        if filter.isEmpty || !inside.isEmpty {
                            GroupHeading(group: group, sheet: $sheet).padding(.top, 2)
                            if group.open || !filter.isEmpty { ForEach(inside) { row($0, indent: 13) } }
                        }
                    }
                }
                .padding(.horizontal, 5).padding(.top, 8)
            }
            Divider()
            HStack {
                Button { sheet = .newGroup } label: { Label("Novo grupo", systemImage: "folder.badge.plus").font(.system(size: 12)) }
                    .buttonStyle(.plain).foregroundStyle(.secondary).help("Novo grupo")
                Spacer()
                Button { shellVerb("settings", ["open": !state.settingsOpen]) } label: { Image(systemName: "gearshape").font(.system(size: 13)) }
                    .buttonStyle(.plain).foregroundStyle(state.settingsOpen ? .primary : .secondary).help("Ajustes")
            }
            .padding(.horizontal, 12).frame(height: 30)
        }
        .frame(width: 260)
        // ponytail: MAGOUI_FLAT is for offscreen renders (ImageRenderer draws an AppKit view as a placeholder); nothing sets it in the app
        .background { if ProcessInfo.processInfo.environment["MAGOUI_FLAT"] == nil { SidebarBackground() } else { Color(white: 0.9) } }
        .overlay(alignment: .trailing) { Divider() }
    }

    @ViewBuilder private func row(_ space: SpaceRow, indent: CGFloat) -> some View {
        SpaceRowView(space: space, state: state, colors: state.spaceColors, groups: side.groups, sheet: $sheet, renaming: $renaming, renamed: $renamed)
            .padding(.leading, indent)
    }
}

private struct Heading: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View { Text(text).font(.system(size: 12)).foregroundStyle(.secondary).padding(.horizontal, 8).frame(height: 20, alignment: .leading) }
}

// MARK: attention and agents

/// "N agentes precisam de você" with Ir para o próximo, or the green notice that nothing waits.
private struct AttentionCard: View {
    let state: ShellState

    var body: some View {
        let waiting = state.sidebar.waiting
        if waiting > 0 {
            VStack(alignment: .leading, spacing: 8) {
                Label(waitingText(waiting), systemImage: "circle.fill").font(.system(size: 13)).foregroundStyle(Color.amber)
                Button {
                    appAction("sidebar", ["action": "next"])
                } label: {
                    HStack {
                        Text("Ir para o próximo").font(.system(size: 13))
                        Spacer()
                        if !state.sidebar.nextKeys.isEmpty {
                            Text(state.sidebar.nextKeys.replacingOccurrences(of: "Shift", with: "⇧").replacingOccurrences(of: "Cmd", with: "⌘").replacingOccurrences(of: "+", with: " "))
                                .font(.system(size: 10, design: .monospaced)).padding(.horizontal, 4)
                                .overlay(RoundedRectangle(cornerRadius: 4).stroke(Color.black.opacity(0.5), lineWidth: 1))
                        }
                    }
                    .foregroundStyle(Color(white: 0.11)).padding(.horizontal, 10).frame(height: 28)
                    .background(RoundedRectangle(cornerRadius: 7).fill(Color.amber))
                }
                .buttonStyle(.plain)
            }
            .padding(10).background(RoundedRectangle(cornerRadius: 10).fill(Color.amber.opacity(0.16)))
        }
    }

    private func waitingText(_ count: Int) -> String {
        switch count {
        case ...1: "1 agente precisa de você"
        default: "\(count) agentes precisam de você"
        }
    }
}

/// "Agentes deste espaço": one row per agent, foldable.
private struct AgentsSection: View {
    let state: ShellState

    var body: some View {
        if !state.sidebar.agents.isEmpty {
            Button {
                appAction("sidebar", ["action": state.sidebar.agentsClosed ? "agents_open" : "agents_close"])
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: state.sidebar.agentsClosed ? "chevron.right" : "chevron.down").font(.system(size: 9)).frame(width: 12)
                    Text("AGENTES DESTE ESPAÇO").font(.system(size: 12))
                    Spacer()
                }
                .foregroundStyle(.secondary).padding(.horizontal, 6).frame(height: 24)
            }
            .buttonStyle(.plain).padding(.top, 6)
            if !state.sidebar.agentsClosed {
                ForEach(state.sidebar.agents) { agent in
                    Button { shellVerb("agent", ["id": agent.id]) } label: { AgentRowView(agent: agent) }.buttonStyle(RowStyle())
                }
            }
        }
    }
}

private struct AgentRowView: View {
    let agent: AgentRow

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: agent.chat ? "bubble.left" : "terminal").font(.system(size: 12)).frame(width: 18).foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 1) {
                Text(agent.name).font(.system(size: 13)).lineLimit(1)
                Text([agent.label, agent.note].compactMap { $0 }.joined(separator: " · "))
                    .font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 0)
            Circle().fill(color).frame(width: 7, height: 7)
        }
        .padding(.horizontal, 8).frame(height: 40)
        .help(agent.mission ?? agent.name)
    }

    private var color: Color {
        switch agent.state {
        case "waiting": .amber
        case "working": .green
        case "ended": .gray.opacity(0.5)
        default: .secondary
        }
    }
}

/// A sidebar row: a faint wash under the pointer.
struct RowStyle: ButtonStyle {
    var current = false
    func makeBody(configuration: Configuration) -> some View {
        RowLabel(configuration: configuration, current: current)
    }
}

private struct RowLabel: View {
    let configuration: ButtonStyleConfiguration
    let current: Bool
    @State private var hovering = false
    var body: some View {
        configuration.label
            .background(RoundedRectangle(cornerRadius: 6).fill(Color.primary.opacity(current ? 0.10 : hovering || configuration.isPressed ? 0.05 : 0)))
            .contentShape(Rectangle())
            .onHover { hovering = $0 }
    }
}

// MARK: spaces and groups

private struct GroupHeading: View {
    let group: GroupRow
    @Binding var sheet: SpaceSheet?

    var body: some View {
        Button {
            appAction("workspace_groups", ["action": "toggle", "name": group.id])
        } label: {
            HStack(spacing: 4) {
                Image(systemName: group.open ? "chevron.down" : "chevron.right").font(.system(size: 9)).frame(width: 12)
                Text(group.name.uppercased()).font(.system(size: 12)).lineLimit(1)
                Spacer()
            }
            .foregroundStyle(.secondary).padding(.horizontal, 6).frame(height: 24)
        }
        .buttonStyle(RowStyle())
        .help("\(group.name): \(group.open ? "Recolher grupo" : "Expandir grupo")")
        .contextMenu {
            Button("Renomear") { sheet = .renameGroup(id: group.id, name: group.name) }
            Button("Apagar Grupo", role: .destructive) { sheet = .removeGroup(id: group.id, name: group.name) }
        }
    }
}

private struct SpaceRowView: View {
    let space: SpaceRow
    let state: ShellState
    let colors: [[Int]]
    let groups: [GroupRow]
    @Binding var sheet: SpaceSheet?
    @Binding var renaming: String?
    @Binding var renamed: String
    @FocusState private var focused: Bool

    var body: some View {
        Button {
            appAction("workspaces", ["action": "switch", "name": space.id])
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "folder.fill").font(.system(size: 12)).foregroundStyle(Color(rgba: space.color)).frame(width: 18)
                if renaming == space.id {
                    TextField("Nome", text: $renamed).textFieldStyle(.roundedBorder).font(.system(size: 14)).focused($focused)
                        .onSubmit { finishRename(keep: true) }
                        .onExitCommand { finishRename(keep: false) }
                        .onAppear { focused = true }
                } else {
                    Text(space.name).font(.system(size: 14)).lineLimit(1)
                    Spacer(minLength: 4)
                    if space.needs > 0 { Label("\(space.needs)", systemImage: "circle.fill").labelStyle(CountStyle(tint: .amber)) }
                    if space.running > 0 { Label("\(space.running)", systemImage: "terminal").labelStyle(CountStyle(tint: .secondary)) }
                    if space.floors > 0 { Image(systemName: "square.3.layers.3d").font(.system(size: 11)).foregroundStyle(.secondary) }
                }
            }
            .padding(.horizontal, 8).frame(height: 28)
        }
        .buttonStyle(RowStyle(current: space.current))
        .help(space.root.isEmpty ? space.name : "\(space.name)\n\(space.root)")
        .contextMenu { menu }
    }

    private func finishRename(keep: Bool) {
        if keep { appAction("workspaces", ["action": "rename", "name": space.id, "new_name": renamed]) }
        renaming = nil
    }

    @ViewBuilder private var menu: some View {
        Button(space.pinned ? "Desfixar" : "Fixar") { appAction("workspace_menu", ["action": space.pinned ? "unpin" : "pin", "name": space.id]) }
        if !groups.isEmpty {
            Menu("Mover para…") {
                if !space.group.isEmpty { Button("Sem Grupo") { appAction("workspace_groups", ["action": "move", "space": space.id, "group": ""]) } }
                ForEach(groups.filter { $0.id != space.group }) { group in
                    Button(group.name) { appAction("workspace_groups", ["action": "move", "space": space.id, "group": group.id]) }
                }
            }
        }
        if !space.root.isEmpty {
            Button("Copiar Caminho") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(space.root, forType: .string)
            }
        }
        Button("Renomear") { (renamed, renaming) = (space.name, space.id) }
        Menu("Cor") {
            ForEach(colors, id: \.self) { color in
                Button { appAction("workspaces", ["action": "color", "name": space.id, "color": hex(color)]) } label: {
                    Label(hex(color), systemImage: color == space.color ? "checkmark.circle.fill" : "circle.fill")
                }
            }
        }
        Divider()
        Button("Duplicar") { appAction("workspace_menu", ["action": "duplicate", "name": space.id]) }
        Button("Exportar Espaço de Trabalho…") { export() }
        Button("Importar Espaço de Trabalho…") { importSpace() }
        Button("Descarregar") { appAction("workspace_menu", ["action": "unload", "name": space.id]) }
        Divider()
        Button("Remover…", role: .destructive) { sheet = .removeSpace(id: space.id, name: space.name, running: space.running) }
    }

    private func hex(_ color: [Int]) -> String { String(format: "#%02x%02x%02x", color[0], color[1], color[2]) }

    private func export() {
        let panel = NSSavePanel()
        panel.title = "Exportar espaço de trabalho"
        panel.nameFieldStringValue = "\(space.name).espaco"
        if panel.runModal() == .OK, let path = panel.url?.path { appAction("workspace_menu", ["action": "export", "name": space.id, "path": path]) }
    }

    private func importSpace() {
        let panel = NSOpenPanel()
        panel.title = "Importar espaço de trabalho"
        if panel.runModal() == .OK, let path = panel.url?.path { appAction("workspace_menu", ["action": "import", "path": path]) }
    }
}

private struct CountStyle: LabelStyle {
    let tint: Color
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 3) {
            configuration.icon.font(.system(size: 8)).foregroundStyle(tint)
            configuration.title.font(.system(size: 11)).foregroundStyle(.secondary)
        }
    }
}
