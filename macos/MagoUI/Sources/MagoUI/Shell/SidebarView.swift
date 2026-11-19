import AppKit
import SwiftUI

/// What the sidebar's and the corners' own sheets ask for (Swift keeps them: the names are typed here and the actions are the app's).
enum SpaceSheet: Identifiable, Equatable {
    case newSpace
    case removeSpace(id: String, name: String, running: Int)
    case newGroup
    case renameGroup(id: String, name: String)
    case removeGroup(id: String, name: String)
    case newFloor

    var id: String {
        switch self {
        case .newSpace: "new-space"
        case .removeSpace(let id, _, _): "remove-space-\(id)"
        case .newGroup: "new-group"
        case .renameGroup(let id, _): "rename-group-\(id)"
        case .removeGroup(let id, _): "remove-group-\(id)"
        case .newFloor: "new-floor"
        }
    }
}

/// The window's left column (`ui/sidebar.rs`): Filtrar and +, Precisa de você, the agents of the space, the spaces (Fixados,
/// loose, groups), and at the foot the plans' use, Novo grupo and Ajustes.
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
            // under the window's buttons, which sit over the sidebar's top (LIGHTS_Y + 22)
            HStack(spacing: 8) {
                HStack(spacing: 0) {
                    IconView(icon: .search, scale: 0.75).foregroundStyle(Tones.muted).frame(width: 12).padding(.leading, 10)
                    TextField("", text: $filter, prompt: Text("Filtrar").foregroundStyle(Tones.muted))
                        .textFieldStyle(.plain).font(.app(14)).foregroundStyle(Tones.text).padding(.leading, 6)
                }
                .frame(width: 199, height: 32)
                .background(Capsule().fill(Tones.hover))
                PlusButton { sheet = .newSpace }
            }
            .padding(.leading, 10).frame(maxWidth: .infinity, alignment: .leading).padding(.top, 54)
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    AttentionCard(state: state)
                    AgentsSection(state: state)
                    Heading("ESPAÇOS DE TRABALHO")
                    if visibleSpaces.isEmpty {
                        Text("Nenhum espaço de trabalho corresponde à busca.").font(.app(13)).foregroundStyle(Tones.muted).padding(.top, 8)
                    }
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
                .padding(.horizontal, 5).padding(.top, 12)
            }
            Rectangle().fill(Tones.line).frame(height: 1)
            if !side.usage.isEmpty { UsageFoot(side: side) }
            Foot(settingsOpen: state.settingsOpen) { sheet = .newGroup }
        }
        .frame(width: 260)
        .background(Tones.ground)
        .overlay(alignment: .trailing) { Rectangle().fill(Tones.line).frame(width: 1) }
    }

    @ViewBuilder private func row(_ space: SpaceRow, indent: CGFloat) -> some View {
        SpaceRowView(space: space, colors: state.spaceColors, groups: side.groups, sheet: $sheet, renaming: $renaming, renamed: $renamed)
            .padding(.leading, indent)
    }
}

/// + beside Filtrar: a new space.
private struct PlusButton: View {
    let action: () -> Void
    @State private var hovering = false
    var body: some View {
        Button(action: action) {
            IconView(icon: .plus, scale: 1.05).foregroundStyle(Tones.text.opacity(hovering ? 1 : 0.7))
                .frame(width: 32, height: 32).background(Circle().fill(Tones.text.opacity(hovering ? 0.1 : 0)))
        }
        .buttonStyle(.plain).onHover { hovering = $0 }.help("Novo espaço de trabalho")
    }
}

/// The last row: Novo grupo, and Ajustes' gear at the right.
private struct Foot: View {
    let settingsOpen: Bool
    let newGroup: () -> Void
    @State private var overGroup = false
    @State private var overGear = false

    var body: some View {
        HStack(spacing: 0) {
            Button(action: newGroup) {
                HStack(spacing: 5) {
                    IconView(icon: .open, scale: 0.7).frame(width: 12)
                    Text("Novo grupo").font(.app(12))
                }
                .foregroundStyle(overGroup ? Tones.text : Tones.muted)
            }
            .buttonStyle(.plain).onHover { overGroup = $0 }.help("Novo grupo")
            Spacer()
            Button { shellVerb("settings", ["open": !settingsOpen]) } label: {
                IconView(icon: .gear, scale: 0.75).foregroundStyle(settingsOpen || overGear ? Tones.text : Tones.muted)
                    .frame(width: 20, height: 20).background(RoundedRectangle(cornerRadius: 5).fill(settingsOpen || overGear ? Tones.hover : .clear))
            }
            .buttonStyle(.plain).onHover { overGear = $0 }.help("Ajustes")
        }
        .padding(.leading, 8).padding(.trailing, 8).frame(height: 30)
    }
}

/// A small uppercase title over a stretch of the list.
private struct Heading: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View { Text(text).font(.app(12)).foregroundStyle(Tones.muted).padding(.leading, 8).frame(height: 20, alignment: .leading) }
}

/// ⋯ at a row's right while the pointer is over it: the row's menu.
private struct MoreMenu<Items: View>: View {
    @ViewBuilder let items: Items
    var body: some View {
        Menu { items } label: { IconView(icon: .more).foregroundStyle(Tones.muted) }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).frame(width: 22, height: 22)
    }
}

// MARK: spaces and groups

private struct GroupHeading: View {
    let group: GroupRow
    @Binding var sheet: SpaceSheet?
    @State private var hovering = false

    var body: some View {
        Button {
            appAction("workspace_groups", ["action": "toggle", "name": group.id])
        } label: {
            HStack(spacing: 0) {
                Chevron(open: group.open).frame(width: 21).padding(.leading, 4)
                Text(group.name.uppercased()).font(.app(12)).foregroundStyle(Tones.muted).lineLimit(1).padding(.leading, 2)
                Spacer(minLength: 4)
            }
            .frame(height: 24)
        }
        .buttonStyle(RowStyle())
        .overlay(alignment: .trailing) { if hovering { MoreMenu { menu }.padding(.trailing, 2) } }
        .onHover { hovering = $0 }
        .help("\(group.name): \(group.open ? "Recolher grupo" : "Expandir grupo")")
        .contextMenu { menu }
    }

    @ViewBuilder private var menu: some View {
        Button("Renomear") { sheet = .renameGroup(id: group.id, name: group.name) }
        Button("Apagar Grupo", role: .destructive) { sheet = .removeGroup(id: group.id, name: group.name) }
    }
}

private struct SpaceRowView: View {
    let space: SpaceRow
    let colors: [[Int]]
    let groups: [GroupRow]
    @Binding var sheet: SpaceSheet?
    @Binding var renaming: String?
    @Binding var renamed: String
    @FocusState private var focused: Bool
    @State private var hovering = false

    var body: some View {
        Button {
            appAction("workspaces", ["action": "switch", "name": space.id])
        } label: {
            HStack(spacing: 0) {
                IconView(icon: .open, scale: 0.8).foregroundStyle(Color(rgba: space.color)).frame(width: 29 - 4).padding(.leading, 2)
                if renaming == space.id {
                    TextField("Nome", text: $renamed).textFieldStyle(.roundedBorder).font(.app(14)).focused($focused)
                        .onSubmit { finishRename(keep: true) }
                        .onExitCommand { finishRename(keep: false) }
                        .onAppear { focused = true }
                        .padding(.trailing, 4)
                } else {
                    Text(space.name).font(.app(14)).foregroundStyle(Tones.text).lineLimit(1).padding(.leading, 2)
                    Spacer(minLength: 4)
                    counts.padding(.trailing, hovering ? 28 : 8)
                }
            }
            .frame(height: 28)
        }
        .buttonStyle(RowStyle(current: space.current))
        .overlay(alignment: .trailing) { if hovering && renaming != space.id { MoreMenu { menu }.padding(.trailing, 2) } }
        .onHover { hovering = $0 }
        .help(tip)
        .contextMenu { menu }
    }

    /// The agents that need you in amber, the ones running by a terminal's mark, and the floors' layers.
    @ViewBuilder private var counts: some View {
        HStack(spacing: 6) {
            if space.needs > 0 {
                HStack(spacing: 3) {
                    Circle().fill(Color.amber).frame(width: 6, height: 6)
                    Text("\(space.needs)").font(.app(11)).foregroundStyle(Tones.muted)
                }
            }
            if space.running > 0 {
                HStack(spacing: 2) {
                    IconView(icon: .tool("terminal"), scale: 0.8).foregroundStyle(Tones.muted).frame(width: 14)
                    Text("\(space.running)").font(.app(11)).foregroundStyle(Tones.muted)
                }
            }
            // ponytail: the layers only mark a space with floors; egui's button there unfolds its floors under the row
            if space.floors > 0 { IconView(icon: .layers(false), scale: 0.75).foregroundStyle(Tones.muted).frame(width: 16) }
        }
    }

    private var tip: String {
        let agents = switch (space.running, space.needs) {
        case (0, 0): "Nenhum agente trabalhando"
        case (0, let needs): "\(needs) agente(s) precisa(m) de você: uma decisão ou uma tarefa que terminou"
        case (let running, 0): "\(running) agente(s) trabalhando"
        case (let running, let needs): "\(running) agente(s) trabalhando, \(needs) precisa(m) de você"
        }
        return space.root.isEmpty ? agents : "\(agents)\n\(space.root)"
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
                    Label { Text(color == space.color ? "\(hex(color)) ✓" : hex(color)) } icon: { Image(nsImage: swatch(color)) }
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

    /// A dot of `color` for the menu (a menu draws its symbols in one ink).
    private func swatch(_ color: [Int]) -> NSImage {
        let image = NSImage(size: NSSize(width: 14, height: 14), flipped: false) { rect in
            NSColor(srgbRed: CGFloat(color[0]) / 255, green: CGFloat(color[1]) / 255, blue: CGFloat(color[2]) / 255, alpha: 1).setFill()
            NSBezierPath(ovalIn: rect.insetBy(dx: 1, dy: 1)).fill()
            return true
        }
        image.isTemplate = false
        return image
    }

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
